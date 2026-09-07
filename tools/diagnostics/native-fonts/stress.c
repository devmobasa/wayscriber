#define _POSIX_C_SOURCE 200809L
#include <cairo.h>
#include <pango/pangocairo.h>
#include <pango/pangofc-font.h>
#include <pango/pangofc-fontmap.h>
#include <fontconfig/fontconfig.h>
#include <pthread.h>
#include <stdatomic.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

/* Headless native-only reproducer: no Rust, retained PangoLayout, custom font
 * map, raw FT_Face use, global shutdown, or shared drawing objects. Only
 * immutable font-description strings and a start barrier cross threads. */
static char **descriptions;
static size_t font_count;
static pthread_barrier_t start_barrier;
static atomic_ulong completed;
static const char *samples[] = {
    "Cache eviction: ABCDEFGHIJKLMNOPQRSTUVWXYZ 0123456789",
    "你好 Καλημέρα onboarding — café שלום العربية",
    "Wrapped body text with café and שלום repeated across the narrow card. More words occupy another line.",
    "A long Unicode checklist label 你好 café 1234567890"
};
struct job { unsigned id, wave, iterations; };

static void *worker(void *arg) {
    struct job *job = arg;
    pthread_barrier_wait(&start_barrier);
    /* Different lifetimes overlap default per-thread font-map teardown with
     * other threads still creating fonts, shaping, and drawing. */
    unsigned count = job->iterations + (job->id % 7) * 3;
    for (unsigned i = 0; i < count; i++) {
        unsigned density = 1 + ((i + job->id) % 2);
        size_t index = (job->id * 17u + job->wave * 11u + i) % font_count;
        cairo_surface_t *surface = cairo_image_surface_create(CAIRO_FORMAT_ARGB32, 420 * density, 240 * density);
        cairo_t *cr = cairo_create(surface);
        cairo_scale(cr, density, density);
        cairo_translate(cr, (double)(i % 3), (double)(i % 5));
        cairo_set_source_rgb(cr, 0.08, 0.1, 0.12);
        cairo_paint(cr);
        cairo_set_source_rgb(cr, 0.9, 0.9, 0.9);
        PangoLayout *layout = pango_cairo_create_layout(cr);
        PangoFontDescription *desc = pango_font_description_from_string(descriptions[index]);
        pango_font_description_set_absolute_size(desc, (12.0 + (i % 9) * 1.3) * PANGO_SCALE);
        pango_layout_set_font_description(layout, desc);
        if (job->wave == 0 && job->id == 0 && i < font_count) {
            PangoFont *font = pango_context_load_font(pango_layout_get_context(layout), desc);
            if (font && PANGO_IS_FC_FONT(font)) {
                FcChar8 *resolved;
                FcPattern *pattern = pango_fc_font_get_pattern(PANGO_FC_FONT(font));
                if (FcPatternGetString(pattern, FC_FILE, 0, &resolved) == FcResultMatch)
                    fprintf(stderr, "resolved[%zu] %s\n", index, resolved);
            }
            if (font) g_object_unref(font);
        }
        pango_layout_set_text(layout, samples[(i + job->id) % 4], -1);
        pango_layout_set_width(layout, (290 + (i % 5) * 10) * PANGO_SCALE);
        pango_layout_set_wrap(layout, PANGO_WRAP_WORD_CHAR);
        /* Metrics followed immediately by show mirrors the failing call chain.
         * Both operations use the same fresh layout and unchanged target. */
        PangoRectangle ink, logical;
        pango_layout_get_extents(layout, &ink, &logical);
        cairo_move_to(cr, 10, 35 - (double)pango_layout_get_baseline(layout) / PANGO_SCALE);
        pango_cairo_show_layout(cr, layout);
        cairo_surface_flush(surface);
        if (cairo_status(cr) != CAIRO_STATUS_SUCCESS || cairo_surface_status(surface) != CAIRO_STATUS_SUCCESS) {
            fprintf(stderr, "Cairo error wave=%u thread=%u iteration=%u\n", job->wave, job->id, i);
            abort();
        }
        pango_font_description_free(desc);
        g_object_unref(layout);
        cairo_destroy(cr);
        cairo_surface_destroy(surface);
        atomic_fetch_add_explicit(&completed, 1, memory_order_relaxed);
    }
    return NULL;
}

static unsigned argument(const char *s, unsigned fallback, unsigned maximum) {
    if (!s) return fallback;
    char *end;
    unsigned long n = strtoul(s, &end, 10);
    if (*end || n == 0 || n > maximum) { fprintf(stderr, "Invalid numeric argument: %s\n", s); exit(2); }
    return (unsigned)n;
}

int main(int argc, char **argv) {
    if (argc > 5) { fprintf(stderr, "usage: %s [threads=24] [waves=100] [iterations=96] [fonts=64]\n", argv[0]); return 2; }
    unsigned threads = argument(argc > 1 ? argv[1] : NULL, 24, 256);
    unsigned waves = argument(argc > 2 ? argv[2] : NULL, 100, 100000);
    unsigned iterations = argument(argc > 3 ? argv[3] : NULL, 96, 100000);
    unsigned limit = argument(argc > 4 ? argv[4] : NULL, 64, 1024);
    if (!FcInit()) return 2;
    FcPattern *query = FcPatternCreate();
    FcPatternAddBool(query, FC_SCALABLE, FcTrue);
    FcObjectSet *objects = FcObjectSetBuild(FC_FAMILY, FC_STYLE, FC_WEIGHT, FC_SLANT, FC_WIDTH, FC_FILE, FC_INDEX, NULL);
    FcFontSet *fonts = FcFontList(NULL, query, objects);
    descriptions = calloc(limit, sizeof(*descriptions));
    char **files = calloc(limit, sizeof(*files));
    if (!fonts || !descriptions || !files) abort();
    for (int i = 0; i < fonts->nfont && font_count < limit; i++) {
        FcChar8 *file;
        if (FcPatternGetString(fonts->fonts[i], FC_FILE, 0, &file) != FcResultMatch) continue;
        int duplicate = 0;
        for (size_t j = 0; j < font_count; j++) if (!strcmp(files[j], (char *)file)) duplicate = 1;
        if (duplicate) continue;
        PangoFontDescription *desc = pango_fc_font_description_from_pattern(fonts->fonts[i], FALSE);
        if (!desc) continue;
        descriptions[font_count] = pango_font_description_to_string(desc);
        files[font_count] = strdup((char *)file);
        fprintf(stderr, "font[%zu] %s | %s\n", font_count, descriptions[font_count], files[font_count]);
        pango_font_description_free(desc);
        font_count++;
    }
    FcFontSetDestroy(fonts); FcObjectSetDestroy(objects); FcPatternDestroy(query);
    if (font_count < 11) { fprintf(stderr, "Need more than ten distinct installed font files; found %zu\n", font_count); return 2; }
    fprintf(stderr, "Pango %s Cairo %s; threads=%u waves=%u iterations=%u distinct-source-files=%zu\n", pango_version_string(), cairo_version_string(), threads, waves, iterations, font_count);
    pthread_t *ids = calloc(threads, sizeof(*ids));
    struct job *jobs = calloc(threads, sizeof(*jobs));
    if (!ids || !jobs) abort();
    for (unsigned wave = 0; wave < waves; wave++) {
        if (pthread_barrier_init(&start_barrier, NULL, threads)) abort();
        for (unsigned t = 0; t < threads; t++) {
            jobs[t] = (struct job){t, wave, iterations};
            if (pthread_create(&ids[t], NULL, worker, &jobs[t])) abort();
        }
        for (unsigned t = 0; t < threads; t++) if (pthread_join(ids[t], NULL)) abort();
        pthread_barrier_destroy(&start_barrier);
        fprintf(stderr, "completed wave=%u draws=%lu\n", wave + 1, atomic_load(&completed));
    }
    free(ids); free(jobs);
    for (size_t i = 0; i < font_count; i++) { g_free(descriptions[i]); free(files[i]); }
    free(descriptions); free(files);
    /* Deliberately no FcFini/cairo_debug_reset_static_data/global Pango reset. */
    return 0;
}
