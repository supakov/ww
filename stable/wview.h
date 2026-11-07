/* wview.h: the stable interface of the plugins of wview, version 1 (docs/plugins.md, 6.9).
 *
 * A plugin in any language with a C FFI is a DLL that exports one function, wv_plugin_get, which
 * gives the table of the plugin. The plugin draws a picture of RGBA pixels and names the controls
 * of its bar; wview draws them, gives it the keys and the mouse of its view, and calls back when a
 * control changes. A plugin of this interface outlives the releases of wview: it is built once.
 *
 * The rules:
 * - Every call comes from the thread of the window; a plugin may run threads of its own.
 * - Every text is UTF-8 and ends with a zero.
 * - What wview gives lives for the call; what the plugin gives (its pictures, controls and texts)
 *   lives until its next call on the same view, so the plugin keeps it in its own memory.
 * - wview refuses an api_version it does not know. A struct with a size grows at its end only:
 *   each side reads the fields the size covers.
 */
#ifndef WVIEW_H
#define WVIEW_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define WV_API_VERSION 1u

/* Modifiers of a key or of the mouse. */
#define WV_SHIFT 1u
#define WV_CTRL 2u
#define WV_ALT 4u

/* The buttons of the mouse. */
#define WV_LEFT 1u
#define WV_RIGHT 2u
#define WV_MIDDLE 4u

/* The keys a view takes while it has the focus, by groups; the others stay the window's. Tab
 * gives the focus back, and so does Escape unless the view takes it. */
#define WV_KEYS_ARROWS 1u   /* ArrowLeft, ArrowRight, ArrowUp, ArrowDown */
#define WV_KEYS_PAGES 2u    /* PageUp, PageDown, Home, End */
#define WV_KEYS_LETTERS 4u  /* "a" to "z" */
#define WV_KEYS_DIGITS 8u   /* "0" to "9" */
#define WV_KEYS_SIGNS 16u   /* "+", "-", "=" */
#define WV_KEYS_SPACE 32u   /* " " */
#define WV_KEYS_ESCAPE 64u  /* Escape */
#define WV_KEYS_ENTER 128u  /* Enter */

/* The cursor over the view. */
#define WV_CURSOR_DEFAULT 0u
#define WV_CURSOR_CROSS 1u
#define WV_CURSOR_HAND 2u
#define WV_CURSOR_GRAB 3u
#define WV_CURSOR_GRABBING 4u
#define WV_CURSOR_ZOOM_IN 5u
#define WV_CURSOR_ZOOM_OUT 6u
#define WV_CURSOR_MOVE 7u

/* The kinds of a control. */
#define WV_BUTTON 0u
#define WV_TOGGLE 1u   /* value 0 or 1 */
#define WV_SLIDER 2u   /* value from min to max by step */
#define WV_CHOICE 3u   /* value the index of one of the choices */
#define WV_LABEL 4u

typedef struct wv_format {
    const char *name;               /* the name of the format */
    const char *const *extensions;  /* without the dot, lower case */
    uint32_t extension_count;
} wv_format;

typedef struct wv_key {
    const char *name;               /* the W3C name: "ArrowLeft", "PageUp", "Escape", "a", "+" */
    uint32_t pressed;               /* 1: pressed (or repeated while held) */
    uint32_t modifiers;             /* WV_SHIFT | WV_CTRL | WV_ALT */
} wv_key;

typedef struct wv_input {
    uint32_t size;                  /* sizeof(wv_input) of wview */
    uint32_t width, height;         /* the view, in pixels */
    float scale;                    /* pixels a point of the window */
    uint32_t focused, hovered;      /* 1 or 0 */
    float mouse_x, mouse_y;         /* the pointer in the pixels of the view, while over it or
                                       dragging */
    uint32_t buttons;               /* the buttons held now: WV_LEFT | WV_RIGHT | WV_MIDDLE */
    uint32_t clicks;                /* the buttons clicked since the last frame */
    uint32_t double_clicks;         /* the buttons clicked twice since the last frame */
    float drag_x, drag_y;           /* the pixels the pointer moved with a button down */
    float wheel_x, wheel_y;         /* lines of the wheel since the last frame, down positive */
    uint32_t modifiers;             /* held now */
    const wv_key *keys;             /* the keys of the view since the last frame */
    uint32_t key_count;
    const char *text;               /* text typed since the last frame (empty in version 1) */
} wv_input;

typedef struct wv_control {
    uint32_t id;                    /* the plugin's own number, given back to control() */
    uint32_t kind;                  /* WV_BUTTON, WV_TOGGLE, WV_SLIDER, WV_CHOICE, WV_LABEL */
    const char *label;
    double value, min, max, step;
    const char *const *choices;     /* the texts of a choice */
    uint32_t choice_count;
} wv_control;

typedef struct wv_output {
    uint32_t size;                  /* sizeof(wv_output) of wview; the rest wview set to zero */
    uint32_t width, height, stride; /* the picture: RGBA, 8 bits a channel, stride in bytes */
    const uint8_t *pixels;
    uint32_t changed;               /* 1: the picture differs from the last one */
    const wv_control *controls;     /* the controls of the bar of the view */
    uint32_t control_count;
    uint32_t keys;                  /* WV_KEYS_* the view takes while it has the focus */
    uint32_t cursor;                /* WV_CURSOR_* */
    uint32_t again;                 /* 1: draw another frame at once, the plugin is at work */
    const char *status;             /* a line of the status bar */
} wv_output;

typedef struct wv_plugin {
    uint32_t api_version;           /* WV_API_VERSION the plugin was written for */
    const char *name;
    const wv_format *formats;
    uint32_t format_count;
    /* Opens a file: a view, or NULL with the reason written into error (error_size bytes). */
    void *(*open)(const char *path, char *error, uint32_t error_size);
    /* A frame of the view: the input since the last one, the picture and the controls now. */
    void (*frame)(void *view, const wv_input *input, wv_output *output);
    /* A control of the bar changed: a button pressed, a toggle, a slider or a choice set. */
    void (*control)(void *view, uint32_t id, double value);
    /* The view closes; the plugin frees it. */
    void (*close)(void *view);
} wv_plugin;

#ifdef _WIN32
#define WV_EXPORT __declspec(dllexport)
#else
#define WV_EXPORT __attribute__((visibility("default")))
#endif

WV_EXPORT const wv_plugin *wv_plugin_get(void);

#ifdef __cplusplus
}
#endif

#endif /* WVIEW_H */
