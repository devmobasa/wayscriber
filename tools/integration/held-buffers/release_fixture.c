/* Preload only into the disposable headless compositor. Hold wl_buffer.release
 * while recording client-scoped release, configure and preferred-scale events. */
#define _GNU_SOURCE
#include <dlfcn.h>
#include <stdint.h>
#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <wayland-server-core.h>

struct held_release {
    struct wl_resource *resource;
    struct wl_event_source *timer;
    struct wl_listener destroyed;
    pid_t pid;
    uint32_t id;
    int rescheduled;
};

static void (*real_post)(struct wl_resource *, uint32_t, union wl_argument *);
static void (*real_post_variadic)(struct wl_resource *, uint32_t, ...);
static unsigned held, delivered, cancelled;

static long long now_ms(void) {
    struct timespec now;
    clock_gettime(CLOCK_MONOTONIC, &now);
    return (long long)now.tv_sec * 1000 + now.tv_nsec / 1000000;
}

static pid_t owner_pid(struct wl_resource *resource) {
    pid_t pid = 0;
    wl_client_get_credentials(wl_resource_get_client(resource), &pid, NULL, NULL);
    return pid;
}

static void resource_destroyed(struct wl_listener *listener, void *data) {
    (void)data;
    struct held_release *pending = wl_container_of(listener, pending, destroyed);
    fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld buffer=%u release=cancelled\n",
            now_ms(), (long)pending->pid, pending->id);
    wl_event_source_remove(pending->timer);
    wl_list_remove(&pending->destroyed.link);
    cancelled++;
    free(pending);
}

static int deliver_release(void *data) {
    struct held_release *pending = data;
    if (!pending->rescheduled) {
        pending->rescheduled = 1;
        const char *schedule_path = getenv("WAYSCRIBER_RELEASE_SCHEDULE_FILE");
        FILE *schedule = schedule_path ? fopen(schedule_path, "r") : NULL;
        if (schedule) {
            long target_pid;
            unsigned first, second, third;
            if (fscanf(schedule, "%ld %u %u %u", &target_pid, &first, &second, &third) == 4
                    && target_pid == (long)pending->pid) {
                int extra = pending->id == first ? 4000 : pending->id == second ? 2000 : 0;
                if (extra) {
                    fclose(schedule);
                    wl_event_source_timer_update(pending->timer, extra);
                    return 0;
                }
            }
            fclose(schedule);
        }
    }
    wl_list_remove(&pending->destroyed.link);
    wl_event_source_remove(pending->timer);
    real_post(pending->resource, 0, NULL);
    fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld buffer=%u release=delivered\n",
            now_ms(), (long)pending->pid, pending->id);
    delivered++;
    free(pending);
    return 0;
}

static int hold_release(struct wl_resource *resource, uint32_t opcode) {
    const char *raw = getenv("WAYSCRIBER_RELEASE_HOLD_MS");
    if (!raw || opcode != 0 || strcmp(wl_resource_get_class(resource), "wl_buffer") != 0)
        return 0;

    char *end;
    long delay = strtol(raw, &end, 10);
    if (*end != '\0' || delay < 1 || delay > 26000)
        abort();

    struct held_release *pending = calloc(1, sizeof(*pending));
    if (!pending)
        abort();
    pending->resource = resource;
    pending->pid = owner_pid(resource);
    pending->id = wl_resource_get_id(resource);
    struct wl_display *display = wl_client_get_display(wl_resource_get_client(resource));
    pending->timer = wl_event_loop_add_timer(wl_display_get_event_loop(display), deliver_release, pending);
    if (!pending->timer)
        abort();
    pending->destroyed.notify = resource_destroyed;
    wl_resource_add_destroy_listener(resource, &pending->destroyed);
    /* The driver identifies the exact held lifetimes after all slots fill;
     * their timers are reordered when each base deadline expires. */
    wl_event_source_timer_update(pending->timer, (int)delay);
    held++;
    fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld buffer=%u release=held delay_ms=%ld\n",
            now_ms(), (long)pending->pid, pending->id, delay);
    return 1;
}

void wl_resource_post_event_array(struct wl_resource *resource, uint32_t opcode, union wl_argument *args) {
    if (!real_post)
        real_post = dlsym(RTLD_NEXT, "wl_resource_post_event_array");

    const char *class = wl_resource_get_class(resource);
    if (opcode == 0 && strcmp(class, "zwlr_layer_surface_v1") == 0 && args)
        fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld layer=%u configure=%ux%u serial=%u\n",
                now_ms(), (long)owner_pid(resource), wl_resource_get_id(resource),
                args[1].u, args[2].u, args[0].u);
    if (opcode == 0 && strcmp(class, "wp_fractional_scale_v1") == 0 && args)
        fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld fractional=%u preferred=%u/120\n",
                now_ms(), (long)owner_pid(resource), wl_resource_get_id(resource), args[0].u);

    if (!hold_release(resource, opcode))
        real_post(resource, opcode, args);
}

/* wlroots 0.17 uses the variadic entry point; newer wlroots uses the array
 * entry point above. GCC's apply builtin forwards unknown protocol signatures
 * unchanged after we inspect the few event types this fixture needs. */
void wl_resource_post_event(struct wl_resource *resource, uint32_t opcode, ...) {
    void *original_arguments = __builtin_apply_args();
    if (!real_post_variadic)
        real_post_variadic = dlsym(RTLD_NEXT, "wl_resource_post_event");
    if (!real_post)
        real_post = dlsym(RTLD_NEXT, "wl_resource_post_event_array");

    const char *class = wl_resource_get_class(resource);
    if (opcode == 0 && strcmp(class, "zwlr_layer_surface_v1") == 0) {
        va_list args;
        va_start(args, opcode);
        unsigned serial = va_arg(args, unsigned);
        unsigned width = va_arg(args, unsigned);
        unsigned height = va_arg(args, unsigned);
        va_end(args);
        fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld layer=%u configure=%ux%u serial=%u\n",
                now_ms(), (long)owner_pid(resource), wl_resource_get_id(resource),
                width, height, serial);
    }
    if (opcode == 0 && strcmp(class, "wp_fractional_scale_v1") == 0) {
        va_list args;
        va_start(args, opcode);
        unsigned preferred = va_arg(args, unsigned);
        va_end(args);
        fprintf(stderr, "held-buffer-fixture t=%lld pid=%ld fractional=%u preferred=%u/120\n",
                now_ms(), (long)owner_pid(resource), wl_resource_get_id(resource), preferred);
    }

    if (!hold_release(resource, opcode))
        __builtin_apply((void (*)())real_post_variadic, original_arguments, 128);
}

__attribute__((destructor)) static void report(void) {
    fprintf(stderr, "held-buffer-fixture summary held=%u delivered=%u cancelled=%u outstanding=%u\n",
            held, delivered, cancelled, held - delivered - cancelled);
}
