/* Isolated Wayland test pointer. Generate the wlr protocol header/code first. */
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>
#include <wayland-client.h>
#include "v5_virtual_pointer_protocol.h"

static struct zwlr_virtual_pointer_manager_v1 *manager;

static void global(void *data, struct wl_registry *registry, uint32_t name,
                   const char *interface, uint32_t version) {
    (void)data;
    if (strcmp(interface, zwlr_virtual_pointer_manager_v1_interface.name) == 0) {
        manager = wl_registry_bind(registry, name,
                                   &zwlr_virtual_pointer_manager_v1_interface,
                                   version < 2 ? version : 2);
    }
}

static void global_remove(void *data, struct wl_registry *registry, uint32_t name) {
    (void)data;
    (void)registry;
    (void)name;
}

static const struct wl_registry_listener registry_listener = {global, global_remove};

static uint32_t now_ms(void) {
    struct timespec time;
    clock_gettime(CLOCK_MONOTONIC, &time);
    return (uint32_t)(time.tv_sec * 1000u + time.tv_nsec / 1000000u);
}

static void move(struct zwlr_virtual_pointer_v1 *pointer, int x, int y,
                 int width, int height) {
    zwlr_virtual_pointer_v1_motion_absolute(pointer, now_ms(), (uint32_t)x,
                                            (uint32_t)y, (uint32_t)width,
                                            (uint32_t)height);
    zwlr_virtual_pointer_v1_frame(pointer);
}

int main(int argc, char **argv) {
    const char *gate = getenv("WAYSCRIBER_ISOLATED_POINTER");
    if (gate == NULL || strcmp(gate, "1") != 0 || argc != 8 ||
        (strcmp(argv[1], "click") != 0 && strcmp(argv[1], "drag") != 0)) {
        fprintf(stderr, "usage: set WAYSCRIBER_ISOLATED_POINTER=1; "
                "%s click|drag x1 y1 x2 y2 width height\n", argv[0]);
        return 2;
    }
    int x1 = atoi(argv[2]), y1 = atoi(argv[3]);
    int x2 = atoi(argv[4]), y2 = atoi(argv[5]);
    int width = atoi(argv[6]), height = atoi(argv[7]);
    if (width <= 0 || height <= 0 || x1 < 0 || y1 < 0 || x2 < 0 || y2 < 0 ||
        x1 >= width || x2 >= width || y1 >= height || y2 >= height) {
        fprintf(stderr, "pointer coordinates exceed isolated output\n");
        return 2;
    }

    struct wl_display *display = wl_display_connect(NULL);
    if (display == NULL) {
        perror("wl_display_connect");
        return 1;
    }
    struct wl_registry *registry = wl_display_get_registry(display);
    wl_registry_add_listener(registry, &registry_listener, NULL);
    wl_display_roundtrip(display);
    if (manager == NULL) {
        fprintf(stderr, "compositor has no wlr virtual pointer manager\n");
        wl_display_disconnect(display);
        return 1;
    }
    struct zwlr_virtual_pointer_v1 *pointer =
        zwlr_virtual_pointer_manager_v1_create_virtual_pointer(manager, NULL);
    wl_display_roundtrip(display);
    move(pointer, x1, y1, width, height);
    wl_display_roundtrip(display);
    zwlr_virtual_pointer_v1_button(pointer, now_ms(), 0x110, 1);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);

    if (strcmp(argv[1], "drag") == 0) {
        for (int step = 1; step <= 10; step++) {
            int x = x1 + (x2 - x1) * step / 10;
            int y = y1 + (y2 - y1) * step / 10;
            usleep(20000);
            move(pointer, x, y, width, height);
            wl_display_roundtrip(display);
        }
    }
    zwlr_virtual_pointer_v1_button(pointer, now_ms(), 0x110, 0);
    zwlr_virtual_pointer_v1_frame(pointer);
    wl_display_roundtrip(display);
    zwlr_virtual_pointer_v1_destroy(pointer);
    zwlr_virtual_pointer_manager_v1_destroy(manager);
    wl_registry_destroy(registry);
    wl_display_disconnect(display);
    return 0;
}
