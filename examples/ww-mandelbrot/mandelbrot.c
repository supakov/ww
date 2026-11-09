/* ww-mandelbrot: the example of the stable interface of W# (ww.h, docs/stable.md).
 *
 * It opens a .mandel file, a few lines of text with the view (centre, zoom, iterations, palette,
 * smooth colours), read through the cache of the host (ww_host), and saves the view back with
 * Save. It opens the same text as some bytes (a source: a part of another file, a file it
 * embeds), finds its type in the first bytes of a file of any name (sniff), and embeds a page of
 * its own and a sample. The picture is drawn in steps: blocks of 8 pixels at once, then of 4, 2
 * and 1, each frame within a few milliseconds, so the window never waits.
 *
 * Its tools (ww_tools_get), for the MCP server of the program and a call by name: how fast a point
 * of the plane leaves the set, the set drawn in letters, and its sample opened in a tab through
 * the tool open_file of the program, called by its name (call_tool of ww_host). The option limit
 * of the plugin is the most iterations a call may ask.
 *
 * The mouse: the wheel zooms at the pointer, a drag moves the picture, Shift and a drag draws a
 * rectangle that the release zooms into, a double click centres and zooms in, the right button
 * zooms out. The keys: the arrows move, + and - zoom, Page Up and Page Down double or halve the
 * iterations, P takes the next palette, Home resets, Escape drops a rectangle.
 *
 * Build (the toolchain of Visual Studio, x64; the CRT linked in, so no DLL of it is needed):
 *   cl /nologo /LD /O2 /MT /W3 /std:c11 /I ..\..\stable mandelbrot.c /Fe:ww_mandelbrot.dll
 */
#define _CRT_SECURE_NO_WARNINGS
#define WIN32_LEAN_AND_MEAN
#include <math.h>
#include <stddef.h>
#include <stdint.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <windows.h>

#include "ww.h"

#define BUDGET_MS 12.0
#define MIN_ITERATIONS 16
#define MAX_ITERATIONS 100000
/* The largest text of a view the plugin reads. */
#define MAX_BYTES 65536

enum {
    C_ITERATIONS = 1,
    C_PALETTE,
    C_SMOOTH,
    C_ZOOM_IN,
    C_ZOOM_OUT,
    C_RESET,
    C_SAVE,
    C_COUNT = 7
};

static const char *const PALETTES[4] = {"classic", "fire", "ice", "grey"};

typedef struct view {
    char path[2048];
    double cx, cy, zoom;
    int iterations, palette, smooth;
    uint32_t width, height;
    uint8_t *picture; /* the picture computed so far */
    uint8_t *shown;   /* the picture with the rectangle drawn on it */
    int step;         /* the size of the blocks of the pass under way, 0 when the picture is done */
    uint32_t row;     /* the next row of the pass */
    int changed;      /* the picture changed since the last frame */
    double started;   /* when the picture began, ms */
    double last_ms;   /* how long the last whole picture took */
    int rect_on;
    float rx0, ry0, rx1, ry1;
    char note[256];   /* what the last save did */
    char status[512];
    ww_control controls[C_COUNT];
} view;

static double now_ms(void) {
    static LARGE_INTEGER frequency;
    LARGE_INTEGER t;
    if (frequency.QuadPart == 0) {
        QueryPerformanceFrequency(&frequency);
    }
    QueryPerformanceCounter(&t);
    return (double)t.QuadPart * 1000.0 / (double)frequency.QuadPart;
}

static void reset(view *v) {
    v->cx = -0.5;
    v->cy = 0.0;
    v->zoom = 1.0;
    v->iterations = 256;
    v->palette = 0;
    v->smooth = 1;
}

/* Starts the picture again, after any change of the view. */
static void restart(view *v) {
    v->step = 8;
    v->row = 0;
    v->started = now_ms();
}

/* The units of the plane a pixel: the whole set fits the view at the zoom 1. */
static double unit(const view *v) {
    double w = v->width ? (double)v->width : 1.0;
    double h = v->height ? (double)v->height : 1.0;
    double a = 3.5 / w, b = 2.6 / h;
    return (a > b ? a : b) / v->zoom;
}

static void plane(const view *v, double px, double py, double *x, double *y) {
    double u = unit(v);
    *x = v->cx + (px - v->width / 2.0) * u;
    *y = v->cy - (py - v->height / 2.0) * u;
}

/* Zooms by a factor keeping the point (px, py) of the view where it is. */
static void zoom_at(view *v, double factor, double px, double py) {
    double x, y, u;
    plane(v, px, py, &x, &y);
    v->zoom *= factor;
    if (v->zoom < 0.1) v->zoom = 0.1;
    if (v->zoom > 1e14) v->zoom = 1e14;
    u = unit(v);
    v->cx = x - (px - v->width / 2.0) * u;
    v->cy = y + (py - v->height / 2.0) * u;
    restart(v);
}

typedef struct stop {
    double at;
    double r, g, b;
} stop;

static const stop CLASSIC[] = {{0.0, 0, 7, 100},      {0.16, 32, 107, 203}, {0.42, 237, 255, 255},
                               {0.6425, 255, 170, 0}, {0.8575, 0, 2, 0},    {1.0, 0, 7, 100}};
static const stop FIRE[] = {{0.0, 20, 0, 0},     {0.3, 180, 20, 0},  {0.55, 255, 120, 0},
                            {0.8, 255, 230, 90}, {0.9, 255, 255, 230}, {1.0, 20, 0, 0}};
static const stop ICE[] = {{0.0, 0, 10, 40},      {0.35, 20, 90, 180}, {0.65, 120, 220, 255},
                           {0.85, 240, 255, 255}, {1.0, 0, 10, 40}};
static const stop GREY[] = {{0.0, 0, 0, 0}, {0.5, 255, 255, 255}, {1.0, 0, 0, 0}};

static void shade(int palette, double t, uint8_t *px) {
    const stop *s;
    int n, i;
    double f;
    switch (palette) {
    case 1: s = FIRE; n = (int)(sizeof FIRE / sizeof FIRE[0]); break;
    case 2: s = ICE; n = (int)(sizeof ICE / sizeof ICE[0]); break;
    case 3: s = GREY; n = (int)(sizeof GREY / sizeof GREY[0]); break;
    default: s = CLASSIC; n = (int)(sizeof CLASSIC / sizeof CLASSIC[0]); break;
    }
    f = fmod(t * 0.025, 1.0);
    if (f < 0) f += 1.0;
    for (i = 0; i + 1 < n; i++) {
        if (f <= s[i + 1].at) break;
    }
    if (i + 1 >= n) i = n - 2;
    {
        double span = s[i + 1].at - s[i].at;
        double k = span > 0 ? (f - s[i].at) / span : 0;
        px[0] = (uint8_t)(s[i].r + (s[i + 1].r - s[i].r) * k);
        px[1] = (uint8_t)(s[i].g + (s[i + 1].g - s[i].g) * k);
        px[2] = (uint8_t)(s[i].b + (s[i + 1].b - s[i].b) * k);
        px[3] = 255;
    }
}

/* The colour of a point of the plane. */
static void color(const view *v, double x0, double y0, uint8_t *px) {
    double x = 0, y = 0, x2 = 0, y2 = 0;
    int n = 0;
    double q = (x0 - 0.25) * (x0 - 0.25) + y0 * y0;
    /* The main cardioid and the bulb of period 2 are inside: no iteration there. */
    if (q * (q + (x0 - 0.25)) <= 0.25 * y0 * y0 || (x0 + 1) * (x0 + 1) + y0 * y0 <= 0.0625) {
        n = v->iterations;
    } else {
        while (x2 + y2 <= 256.0 && n < v->iterations) {
            y = 2 * x * y + y0;
            x = x2 - y2 + x0;
            x2 = x * x;
            y2 = y * y;
            n++;
        }
    }
    if (n >= v->iterations) {
        px[0] = px[1] = px[2] = 0;
        px[3] = 255;
        return;
    }
    {
        double t = n;
        if (v->smooth) {
            double log_zn = log(x2 + y2) / 2.0;
            double nu = log(log_zn / log(2.0)) / log(2.0);
            t = n + 1 - nu;
        }
        shade(v->palette, t, px);
    }
}

/* Computes rows of the pass under way until the time of a frame is spent. */
static void render(view *v) {
    double until = now_ms() + BUDGET_MS;
    if (!v->picture || v->step == 0) return;
    while (v->step > 0) {
        uint32_t s = (uint32_t)v->step;
        while (v->row < v->height) {
            uint32_t xx, yy, x;
            for (x = 0; x < v->width; x += s) {
                uint8_t px[4];
                double re, im;
                plane(v, x + s / 2.0, v->row + s / 2.0, &re, &im);
                color(v, re, im, px);
                for (yy = v->row; yy < v->row + s && yy < v->height; yy++) {
                    for (xx = x; xx < x + s && xx < v->width; xx++) {
                        memcpy(v->picture + ((size_t)yy * v->width + xx) * 4, px, 4);
                    }
                }
            }
            v->row += s;
            v->changed = 1;
            if (now_ms() > until) return;
        }
        v->row = 0;
        v->step /= 2;
        if (v->step == 0) {
            v->last_ms = now_ms() - v->started;
        }
    }
}

static void resize(view *v, uint32_t w, uint32_t h) {
    size_t bytes = (size_t)w * h * 4;
    if (w == v->width && h == v->height && v->picture) return;
    free(v->picture);
    free(v->shown);
    v->width = w;
    v->height = h;
    v->picture = (uint8_t *)calloc(bytes ? bytes : 4, 1);
    v->shown = (uint8_t *)calloc(bytes ? bytes : 4, 1);
    restart(v);
}

/* The picture with the outline of the rectangle of a Shift drag. */
static const uint8_t *with_rectangle(view *v) {
    int x0, y0, x1, y1, x, y;
    if (!v->rect_on) return v->picture;
    memcpy(v->shown, v->picture, (size_t)v->width * v->height * 4);
    x0 = (int)(v->rx0 < v->rx1 ? v->rx0 : v->rx1);
    x1 = (int)(v->rx0 < v->rx1 ? v->rx1 : v->rx0);
    y0 = (int)(v->ry0 < v->ry1 ? v->ry0 : v->ry1);
    y1 = (int)(v->ry0 < v->ry1 ? v->ry1 : v->ry0);
    if (x0 < 0) x0 = 0;
    if (y0 < 0) y0 = 0;
    if (x1 >= (int)v->width) x1 = (int)v->width - 1;
    if (y1 >= (int)v->height) y1 = (int)v->height - 1;
    for (y = y0; y <= y1; y++) {
        for (x = x0; x <= x1; x++) {
            if (x == x0 || x == x1 || y == y0 || y == y1) {
                uint8_t *p = v->shown + ((size_t)y * v->width + x) * 4;
                p[0] = 255;
                p[1] = 255;
                p[2] = 255;
                p[3] = 255;
            }
        }
    }
    return v->shown;
}

/* The table of the host, given to ww_plugin_get and ww_tools_get. */
static const ww_host *HOST;

/* Whether the table of the host holds a field. */
#define HOST_HAS(field) \
    (HOST && HOST->size >= offsetof(ww_host, field) + sizeof(HOST->field) && HOST->field)

/* A file of a path of UTF-8, as the host gives it: a name outside ASCII opens too. */
static FILE *open_utf8(const char *path, const wchar_t *mode) {
    wchar_t wide[2048];
    if (!MultiByteToWideChar(CP_UTF8, 0, path, -1, wide, 2048)) return NULL;
    return _wfopen(wide, mode);
}

static void save(view *v) {
    FILE *f;
    if (!v->path[0]) {
        snprintf(v->note, sizeof v->note, "not saved: some bytes, no file");
        return;
    }
    f = open_utf8(v->path, L"w");
    if (!f) {
        snprintf(v->note, sizeof v->note, "not saved: %s", v->path);
        return;
    }
    fprintf(f, "center_x = %.17g\ncenter_y = %.17g\nzoom = %.17g\niterations = %d\n", v->cx,
            v->cy, v->zoom, v->iterations);
    fprintf(f, "palette = %s\nsmooth = %d\n", PALETTES[v->palette], v->smooth);
    fclose(f);
    snprintf(v->note, sizeof v->note, "saved");
}

/* The view of the lines of a text: "key = value" each. */
static void read_view(view *v, const char *text) {
    const char *line = text;
    while (*line) {
        char buffer[256], key[64], value[128];
        const char *end = strchr(line, '\n');
        size_t n = end ? (size_t)(end - line) : strlen(line);
        if (n >= sizeof buffer) n = sizeof buffer - 1;
        memcpy(buffer, line, n);
        buffer[n] = 0;
        if (sscanf(buffer, " %63[a-z_] = %127s", key, value) == 2) {
            if (!strcmp(key, "center_x")) v->cx = atof(value);
            else if (!strcmp(key, "center_y")) v->cy = atof(value);
            else if (!strcmp(key, "zoom")) v->zoom = atof(value);
            else if (!strcmp(key, "iterations")) v->iterations = atoi(value);
            else if (!strcmp(key, "smooth")) v->smooth = atoi(value) != 0;
            else if (!strcmp(key, "palette")) {
                int i;
                for (i = 0; i < 4; i++) {
                    if (!strcmp(value, PALETTES[i])) v->palette = i;
                }
            }
        }
        if (!end) break;
        line = end + 1;
    }
    if (!(v->zoom > 0.1)) v->zoom = 0.1;
    if (v->zoom > 1e14) v->zoom = 1e14;
    if (v->iterations < MIN_ITERATIONS) v->iterations = MIN_ITERATIONS;
    if (v->iterations > MAX_ITERATIONS) v->iterations = MAX_ITERATIONS;
    if (!isfinite(v->cx) || !isfinite(v->cy)) {
        v->cx = -0.5;
        v->cy = 0.0;
    }
}

/* The bytes of a source of the host as a text that ends with a zero, or NULL with the reason. */
static char *read_source(const ww_source *s, char *error, uint32_t error_size) {
    uint64_t done = 0;
    char *text;
    if (s->length > MAX_BYTES) {
        snprintf(error, error_size, "%llu bytes, more than a view of the set takes",
                 (unsigned long long)s->length);
        return NULL;
    }
    text = (char *)malloc((size_t)s->length + 1);
    if (!text) {
        snprintf(error, error_size, "no memory");
        return NULL;
    }
    while (done < s->length) {
        uint32_t n = s->read(s->self, done, (uint8_t *)text + done, (uint32_t)(s->length - done));
        if (n == 0) break;
        done += n;
    }
    text[done] = 0;
    return text;
}

/* The text of a file: through the cache of the host when it reads files, else read here. */
static char *read_file(const char *path, char *error, uint32_t error_size) {
    if (HOST_HAS(open_file) && HOST_HAS(close_source)) {
        const ww_source *s = HOST->open_file(path, error, error_size);
        char *text;
        if (!s) return NULL;
        text = read_source(s, error, error_size);
        HOST->close_source(s);
        return text;
    } else {
        FILE *f = open_utf8(path, L"rb");
        char *text = (char *)calloc(1, MAX_BYTES + 1);
        if (!f || !text) {
            if (f) fclose(f);
            free(text);
            snprintf(error, error_size, "the file does not open: %s", path);
            return NULL;
        }
        fread(text, 1, MAX_BYTES, f);
        fclose(f);
        return text;
    }
}

/* A view of a text; the path is the file Save writes, empty for some bytes. */
static void *view_of(const char *text, const char *path, char *error, uint32_t error_size) {
    view *v = (view *)calloc(1, sizeof(view));
    if (!v) {
        snprintf(error, error_size, "no memory");
        return NULL;
    }
    snprintf(v->path, sizeof v->path, "%s", path);
    reset(v);
    read_view(v, text);
    restart(v);
    return v;
}

static void *ww_open(const char *path, char *error, uint32_t error_size) {
    char *text = read_file(path, error, error_size);
    void *v;
    if (!text) return NULL;
    v = view_of(text, path, error, error_size);
    free(text);
    return v;
}

/* Some bytes as a view: a part of another file, a file of the set the plugin embeds. */
static void *ww_open_bytes(const ww_source *source, const char *name, const char *mime,
                           char *error, uint32_t error_size) {
    char *text = read_source(source, error, error_size);
    view *v;
    (void)mime;
    if (!text) return NULL;
    v = (view *)view_of(text, "", error, error_size);
    free(text);
    if (v) snprintf(v->note, sizeof v->note, "from %s", name);
    return v;
}

static const char *const MIMES[1] = {"application/x-mandelbrot"};

/* The type of the first bytes: a text with the lines of a view. */
static const char *ww_sniff(const uint8_t *bytes, uint32_t count) {
    char head[513];
    uint32_t n = count < 512 ? count : 512;
    memcpy(head, bytes, n);
    head[n] = 0;
    return strstr(head, "center_x") && strstr(head, "zoom") ? MIMES[0] : NULL;
}

static void set_iterations(view *v, double value) {
    int n = (int)value;
    if (n < MIN_ITERATIONS) n = MIN_ITERATIONS;
    if (n > MAX_ITERATIONS) n = MAX_ITERATIONS;
    v->iterations = n;
    restart(v);
}

static void ww_control_(void *p, uint32_t id, double value) {
    view *v = (view *)p;
    switch (id) {
    case C_ITERATIONS: set_iterations(v, value); break;
    case C_PALETTE: v->palette = ((int)value % 4 + 4) % 4; restart(v); break;
    case C_SMOOTH: v->smooth = value != 0; restart(v); break;
    case C_ZOOM_IN: zoom_at(v, 2.0, v->width / 2.0, v->height / 2.0); break;
    case C_ZOOM_OUT: zoom_at(v, 0.5, v->width / 2.0, v->height / 2.0); break;
    case C_RESET: reset(v); restart(v); break;
    case C_SAVE: save(v); break;
    default: break;
    }
}

static void keys(view *v, const ww_input *in) {
    uint32_t i;
    double u = unit(v);
    for (i = 0; i < in->key_count; i++) {
        const char *k = in->keys[i].name;
        if (!in->keys[i].pressed || !k) continue;
        if (!strcmp(k, "ArrowLeft")) { v->cx -= 0.1 * v->width * u; restart(v); }
        else if (!strcmp(k, "ArrowRight")) { v->cx += 0.1 * v->width * u; restart(v); }
        else if (!strcmp(k, "ArrowUp")) { v->cy += 0.1 * v->height * u; restart(v); }
        else if (!strcmp(k, "ArrowDown")) { v->cy -= 0.1 * v->height * u; restart(v); }
        else if (!strcmp(k, "+") || !strcmp(k, "=")) zoom_at(v, 1.5, v->width / 2.0, v->height / 2.0);
        else if (!strcmp(k, "-")) zoom_at(v, 1.0 / 1.5, v->width / 2.0, v->height / 2.0);
        else if (!strcmp(k, "PageUp")) set_iterations(v, v->iterations * 2.0);
        else if (!strcmp(k, "PageDown")) set_iterations(v, v->iterations / 2.0);
        else if (!strcmp(k, "p")) { v->palette = (v->palette + 1) % 4; restart(v); }
        else if (!strcmp(k, "Home")) { reset(v); restart(v); }
        else if (!strcmp(k, "Escape")) v->rect_on = 0;
        u = unit(v);
    }
}

static void mouse(view *v, const ww_input *in) {
    int shift = (in->modifiers & WW_SHIFT) != 0;
    if (in->hovered && in->wheel_y != 0) {
        zoom_at(v, pow(1.25, -in->wheel_y), in->mouse_x, in->mouse_y);
    }
    if (in->double_clicks & WW_LEFT) {
        double x, y;
        plane(v, in->mouse_x, in->mouse_y, &x, &y);
        v->cx = x;
        v->cy = y;
        zoom_at(v, 2.0, v->width / 2.0, v->height / 2.0);
    } else if (in->clicks & WW_RIGHT) {
        zoom_at(v, 0.5, in->mouse_x, in->mouse_y);
    }
    if (in->buttons & WW_LEFT) {
        if (shift || v->rect_on) {
            if (!v->rect_on) {
                v->rect_on = 1;
                v->rx0 = in->mouse_x - in->drag_x;
                v->ry0 = in->mouse_y - in->drag_y;
            }
            v->rx1 = in->mouse_x;
            v->ry1 = in->mouse_y;
            v->changed = 1;
        } else if (in->drag_x != 0 || in->drag_y != 0) {
            double u = unit(v);
            v->cx -= in->drag_x * u;
            v->cy += in->drag_y * u;
            restart(v);
        }
    } else if (v->rect_on) {
        /* The release of a Shift drag: the rectangle fills the view. */
        double w = fabs(v->rx1 - v->rx0), h = fabs(v->ry1 - v->ry0);
        v->rect_on = 0;
        v->changed = 1;
        if (w > 4 && h > 4) {
            double x, y, k;
            plane(v, (v->rx0 + v->rx1) / 2.0, (v->ry0 + v->ry1) / 2.0, &x, &y);
            k = v->width / w < v->height / h ? v->width / w : v->height / h;
            v->cx = x;
            v->cy = y;
            zoom_at(v, k, v->width / 2.0, v->height / 2.0);
        }
    }
}

static void ww_frame(void *p, const ww_input *in, ww_output *out) {
    view *v = (view *)p;
    uint32_t w = in->width ? in->width : 1, h = in->height ? in->height : 1;
    resize(v, w, h);
    keys(v, in);
    mouse(v, in);
    render(v);
    v->controls[0] = (ww_control){C_ITERATIONS, WW_SLIDER, "Iterations", v->iterations,
                                  MIN_ITERATIONS, 4096, 16, NULL, 0};
    v->controls[1] = (ww_control){C_PALETTE, WW_CHOICE, "Palette", v->palette, 0, 3, 1, PALETTES, 4};
    v->controls[2] = (ww_control){C_SMOOTH, WW_TOGGLE, "Smooth colours", v->smooth, 0, 1, 1, NULL, 0};
    v->controls[3] = (ww_control){C_ZOOM_IN, WW_BUTTON, "Zoom in", 0, 0, 0, 0, NULL, 0};
    v->controls[4] = (ww_control){C_ZOOM_OUT, WW_BUTTON, "Zoom out", 0, 0, 0, 0, NULL, 0};
    v->controls[5] = (ww_control){C_RESET, WW_BUTTON, "Reset", 0, 0, 0, 0, NULL, 0};
    v->controls[6] = (ww_control){C_SAVE, WW_BUTTON, "Save", 0, 0, 0, 0, NULL, 0};
    if (v->step == 0) {
        snprintf(v->status, sizeof v->status,
                 "%.15g %+.15gi, zoom %.4gx, %d iterations, %s, %.1f ms%s%s", v->cx, v->cy,
                 v->zoom, v->iterations, PALETTES[v->palette], v->last_ms, v->note[0] ? ", " : "",
                 v->note);
    } else {
        snprintf(v->status, sizeof v->status, "%.15g %+.15gi, zoom %.4gx, %d iterations, drawing",
                 v->cx, v->cy, v->zoom, v->iterations);
    }
    out->width = v->width;
    out->height = v->height;
    out->stride = v->width * 4;
    out->pixels = with_rectangle(v);
    out->changed = (uint32_t)v->changed;
    v->changed = 0;
    out->controls = v->controls;
    out->control_count = C_COUNT;
    out->keys = WW_KEYS_ARROWS | WW_KEYS_PAGES | WW_KEYS_LETTERS | WW_KEYS_SIGNS |
                (v->rect_on ? WW_KEYS_ESCAPE : 0);
    if (v->rect_on || (in->modifiers & WW_SHIFT)) out->cursor = WW_CURSOR_CROSS;
    else if (in->buttons & WW_LEFT) out->cursor = WW_CURSOR_GRABBING;
    else out->cursor = WW_CURSOR_GRAB;
    out->again = v->step != 0;
    out->status = v->status;
}

static void ww_close(void *p) {
    view *v = (view *)p;
    if (!v) return;
    free(v->picture);
    free(v->shown);
    free(v);
}

/* The page and the sample the plugin embeds. */
static const char GUIDE[] =
    "# The Mandelbrot set (C)\n"
    "\n"
    "A plugin of the stable interface of W# written in C (`ww.h`). It opens `.mandel` files,\n"
    "and any text with the lines of a view, and draws the set in steps.\n"
    "\n"
    "- The wheel zooms at the pointer, a drag moves the picture, Shift and a drag draws a\n"
    "  rectangle the release zooms into, a double click centres and zooms in, the right button\n"
    "  zooms out.\n"
    "- The arrows move, `+` and `-` zoom, Page Up and Page Down double or halve the iterations,\n"
    "  `P` takes the next palette, Home resets.\n"
    "- Save writes the view back into its file.\n"
    "\n"
    "Open [the valley of the sea horses](samples/seahorse.mandel), a file the plugin embeds.\n";

static const char SAMPLE[] =
    "center_x = -0.743643887037151\n"
    "center_y = 0.13182590420533\n"
    "zoom = 180\n"
    "iterations = 1024\n"
    "palette = classic\n"
    "smooth = 1\n";

static const ww_file FILES[2] = {
    {"guide.md", (const uint8_t *)GUIDE, sizeof GUIDE - 1},
    {"samples/seahorse.mandel", (const uint8_t *)SAMPLE, sizeof SAMPLE - 1},
};

static const char *const EXTENSIONS[1] = {"mandel"};
static const ww_format FORMATS[1] = {{"Mandelbrot set", EXTENSIONS, 1, MIMES, 1}};
static const ww_plugin PLUGIN = {
    WW_API_VERSION, sizeof(ww_plugin), "Mandelbrot set (C)", FORMATS, 1, FILES, 2,
    ww_open, ww_open_bytes, ww_sniff, ww_frame, ww_control_, ww_close};

WW_EXPORT const ww_plugin *ww_plugin_get(const ww_host *host) {
    HOST = host;
    return &PLUGIN;
}

/* The tools. A call comes on any thread of the host, several at once: they read nothing but
 * their arguments. */

/* The number of a key of a JSON object as an agent writes it ("key": 1.5, or "key": "1.5" as the
 * options of the server give their values), else fallback. */
static double json_number(const char *json, const char *key, double fallback) {
    char quoted[72];
    const char *at;
    char *end;
    double value;
    if (!json || strlen(key) > 64) return fallback;
    snprintf(quoted, sizeof quoted, "\"%s\"", key);
    at = strstr(json, quoted);
    if (!at) return fallback;
    at = strchr(at + strlen(quoted), ':');
    if (!at) return fallback;
    at++;
    while (*at == ' ' || *at == '"') at++;
    value = strtod(at, &end);
    return end == at ? fallback : value;
}

/* An answer the host frees with tool_free. */
static char *answer_of(const char *text) {
    size_t n = strlen(text) + 1;
    char *out = (char *)malloc(n);
    if (out) memcpy(out, text, n);
    return out;
}

/* The iterations after which a point leaves the set, or -1 when it stays inside. */
static int escape_of(double x0, double y0, int most) {
    double x = 0.0, y = 0.0;
    int i;
    for (i = 0; i < most; i++) {
        double xx = x * x, yy = y * y;
        if (xx + yy > 4.0) return i;
        y = 2.0 * x * y + y0;
        x = xx - yy + x0;
    }
    return -1;
}

static int clamped(double value, int low, int high) {
    if (!(value >= low)) return low;
    if (value > high) return high;
    return (int)value;
}

/* A text as a string of JSON, without its quotes: backslashes and quotes escaped. */
static void json_escaped(const char *text, char *out, size_t size) {
    size_t n = 0;
    for (; *text && n + 2 < size; text++) {
        if (*text == '\\' || *text == '"') out[n++] = '\\';
        out[n++] = *text;
    }
    out[n] = 0;
}

/* mandelbrot_open: the sample into the temporary folder, opened by the tool open_file of the
 * program, called by its name as the plugin. */
static int32_t open_sample(char **answer) {
    char dir[MAX_PATH], path[MAX_PATH + 32], escaped[2 * MAX_PATH + 64], args[2 * MAX_PATH + 96];
    char *opened = NULL;
    int32_t code;
    FILE *f;
    if (!HOST_HAS(call_tool) || !HOST_HAS(free_text)) {
        *answer = answer_of("the host calls no tool");
        return WW_TOOL_FAILED;
    }
    if (!GetTempPathA(MAX_PATH, dir)) {
        *answer = answer_of("no temporary folder");
        return WW_TOOL_WRITE_FAILED;
    }
    snprintf(path, sizeof path, "%sseahorse.mandel", dir);
    f = fopen(path, "wb");
    if (!f) {
        *answer = answer_of("the sample was not written");
        return WW_TOOL_WRITE_FAILED;
    }
    fwrite(SAMPLE, 1, sizeof SAMPLE - 1, f);
    fclose(f);
    json_escaped(path, escaped, sizeof escaped);
    snprintf(args, sizeof args, "{\"path\": \"%s\"}", escaped);
    code = HOST->call_tool(HOST->self, "open_file", args, &opened);
    *answer = answer_of(opened ? opened : "");
    if (opened) HOST->free_text(opened);
    return code;
}

static int32_t tool_call(const char *tool, const char *args, const char *options, char **answer) {
    int limit = (int)json_number(options, "limit", 1000);
    char text[256];
    if (strcmp(tool, "mandelbrot_escape") == 0) {
        double x = json_number(args, "x", NAN), y = json_number(args, "y", NAN);
        int most = clamped(json_number(args, "iterations", 256), 1, MAX_ITERATIONS);
        int n;
        if (isnan(x) || isnan(y)) {
            *answer = answer_of("the arguments x and y are numbers");
            return WW_TOOL_INVALID;
        }
        if (most > limit) {
            snprintf(text, sizeof text,
                     "at most %d iterations: the option limit of the plugin allows no more", limit);
            *answer = answer_of(text);
            return WW_TOOL_NOT_ALLOWED;
        }
        n = escape_of(x, y, most);
        if (n < 0)
            snprintf(text, sizeof text, "{\"inside\": true, \"iterations\": %d}", most);
        else
            snprintf(text, sizeof text, "{\"inside\": false, \"escaped_after\": %d}", n);
        *answer = answer_of(text);
        return WW_TOOL_OK;
    }
    if (strcmp(tool, "mandelbrot_ascii") == 0) {
        static const char RAMP[] = " .:-=+*#%@";
        double cx = json_number(args, "center_x", -0.5), cy = json_number(args, "center_y", 0.0);
        double zoom = json_number(args, "zoom", 1.0);
        int w = clamped(json_number(args, "width", 64), 8, 160);
        int h = clamped(json_number(args, "height", 24), 4, 80);
        int most = limit < 200 ? limit : 200;
        size_t size = (size_t)(w + 8) * (size_t)h + 256, n;
        char *out;
        int row, col;
        if (!(zoom > 0.0)) {
            *answer = answer_of("the zoom is a number above 0");
            return WW_TOOL_INVALID;
        }
        out = (char *)malloc(size);
        if (!out) {
            *answer = answer_of("no memory");
            return WW_TOOL_FAILED;
        }
        n = (size_t)snprintf(out, size, "{\"iterations\": %d, \"lines\": [", most);
        for (row = 0; row < h; row++) {
            /* A letter is about twice as high as it is wide. */
            double y = cy + (row - h / 2.0) * 2.0 * 3.0 / (zoom * w);
            out[n++] = '"';
            for (col = 0; col < w; col++) {
                double x = cx + (col - w / 2.0) * 3.0 / (zoom * w);
                int e = escape_of(x, y, most);
                out[n++] = e < 0 ? RAMP[9] : RAMP[(e * 9) / most];
            }
            out[n++] = '"';
            if (row + 1 < h) out[n++] = ',';
        }
        snprintf(out + n, size - n, "]}");
        *answer = out;
        return WW_TOOL_OK;
    }
    if (strcmp(tool, "mandelbrot_open") == 0) return open_sample(answer);
    *answer = answer_of("no such tool");
    return WW_TOOL_NOT_FOUND;
}

static void tool_free(char *answer) { free(answer); }

static const ww_tool TOOLS[3] = {
    {"mandelbrot_escape",
     "Counts the iterations after which the point x + iy of the plane leaves the Mandelbrot set, "
     "or says that it stays inside.",
     "{\"type\": \"object\", \"properties\": {\"x\": {\"type\": \"number\"}, \"y\": {\"type\": "
     "\"number\"}, \"iterations\": {\"type\": \"integer\", \"description\": \"The most "
     "iterations, 256 by default\"}}, \"required\": [\"x\", \"y\"]}",
     "mandelbrot", WW_MODE_READ, NULL},
    {"mandelbrot_ascii",
     "Draws the Mandelbrot set in letters around a centre at a zoom: a text a row, the darkest "
     "letter inside the set.",
     "{\"type\": \"object\", \"properties\": {\"center_x\": {\"type\": \"number\"}, "
     "\"center_y\": {\"type\": \"number\"}, \"zoom\": {\"type\": \"number\"}, \"width\": "
     "{\"type\": \"integer\"}, \"height\": {\"type\": \"integer\"}}}",
     "mandelbrot", WW_MODE_READ, NULL},
    {"mandelbrot_open",
     "Writes the sample view of the sea horses into the temporary folder and opens it in a tab of "
     "the window, through the tool open_file of the program.",
     "{\"type\": \"object\", \"properties\": {}}", "mandelbrot", WW_MODE_FILES, NULL},
};

static const char *const LIMITS[3] = {"100", "1000", "10000"};
static const char *const LIMIT_NAMES[3] = {"100 iterations", "1000 iterations",
                                           "10000 iterations"};
static const ww_option OPTIONS[1] = {
    {"limit", "The most iterations a call may ask", LIMITS, LIMIT_NAMES, 3, "1000", NULL}};
static const ww_tools TOOLS_TABLE = {WW_API_VERSION, sizeof(ww_tools), "Mandelbrot set (C)",
                                     TOOLS, 3, OPTIONS, 1, tool_call, tool_free};

WW_EXPORT const ww_tools *ww_tools_get(const ww_host *host) {
    HOST = host;
    return &TOOLS_TABLE;
}
