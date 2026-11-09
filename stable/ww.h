/* ww.h: the stable interface of the plugins of W#, version 1 (docs/stable.md).
 *
 * A plugin in any language with a C FFI is a DLL that exports ww_plugin_get, which takes the
 * table of the host and gives the table of the plugin, or ww_tools_get, or both. The plugin names its formats
 * (their extensions and MIME types), may find its types in the first bytes of a file (sniff) and
 * embed files of its own (a guide, samples, translations); a view of a file or of some bytes
 * draws a picture of RGBA pixels and names the controls of its bar. The host draws them, gives the
 * view the keys and the mouse of its area, and calls back when a control changes. A view of some
 * bytes reads them through a source, a part of a file the host reads in blocks through its cache,
 * the same cache the plugin may read any file through (ww_host). A plugin may also have tools
 * (ww_tools_get, a second function, with or without ww_plugin_get): the program serves them to AI
 * agents over MCP and calls them by name, and the plugin calls any tool of the program by its
 * name (call_tool of ww_host). A plugin of this interface outlives the releases of the host: it
 * is built once.
 *
 * The rules:
 * - Every call comes from the thread of the window, but the functions of ww_host and the read of
 *   a source, which any thread of the plugin may call, and the call of a tool, which any thread
 *   of the host makes, several at once; a plugin may run threads of its own.
 * - Every text is UTF-8 and ends with a zero.
 * - What the host gives for a call (an input, a name) lives for the call; a source given to
 *   open_bytes lives until close of its view; a source of open_file or slice until close_source;
 *   the table of the host for the whole run. What the plugin gives (its pictures, controls and
 *   texts) lives until its next call on the same view; its table, formats and files for the run.
 * - The host refuses an api_version it does not know. A struct with a size grows at its end
 *   only: each side reads the fields the size covers and takes the others as zero.
 *
 * The version stays 1 until the first release of W#: until then the structures may change.
 */
#ifndef WW_H
#define WW_H

#include <stdint.h>

#ifdef __cplusplus
extern "C" {
#endif

#define WW_API_VERSION 1u

/* Modifiers of a key or of the mouse. */
#define WW_SHIFT 1u
#define WW_CTRL 2u
#define WW_ALT 4u

/* The buttons of the mouse. */
#define WW_LEFT 1u
#define WW_RIGHT 2u
#define WW_MIDDLE 4u

/* The keys a view takes while it has the focus, by groups; the others stay the window's. Tab
 * gives the focus back, and so does Escape unless the view takes it. */
#define WW_KEYS_ARROWS 1u   /* ArrowLeft, ArrowRight, ArrowUp, ArrowDown */
#define WW_KEYS_PAGES 2u    /* PageUp, PageDown, Home, End */
#define WW_KEYS_LETTERS 4u  /* "a" to "z" */
#define WW_KEYS_DIGITS 8u   /* "0" to "9" */
#define WW_KEYS_SIGNS 16u   /* "+", "-", "=" */
#define WW_KEYS_SPACE 32u   /* " " */
#define WW_KEYS_ESCAPE 64u  /* Escape */
#define WW_KEYS_ENTER 128u  /* Enter */

/* The cursor over the view. */
#define WW_CURSOR_DEFAULT 0u
#define WW_CURSOR_CROSS 1u
#define WW_CURSOR_HAND 2u
#define WW_CURSOR_GRAB 3u
#define WW_CURSOR_GRABBING 4u
#define WW_CURSOR_ZOOM_IN 5u
#define WW_CURSOR_ZOOM_OUT 6u
#define WW_CURSOR_MOVE 7u

/* What a call of a tool gives: WW_TOOL_OK, or the kind of its error. */
#define WW_TOOL_OK 0
#define WW_TOOL_INVALID 1       /* the arguments are wrong */
#define WW_TOOL_NOT_FOUND 2
#define WW_TOOL_NOT_ALLOWED 3   /* the mode or an option of the server forbids it */
#define WW_TOOL_READ_FAILED 4
#define WW_TOOL_WRITE_FAILED 5
#define WW_TOOL_FAILED 6

/* The mode of the server a tool needs: read changes nothing, edit changes what the program holds
 * in memory, files writes files too. */
#define WW_MODE_READ 0u
#define WW_MODE_EDIT 1u
#define WW_MODE_FILES 2u

/* The kinds of a control. */
#define WW_BUTTON 0u
#define WW_TOGGLE 1u   /* value 0 or 1 */
#define WW_SLIDER 2u   /* value from min to max by step */
#define WW_CHOICE 3u   /* value the index of one of the choices */
#define WW_LABEL 4u

typedef struct ww_format {
    const char *name;               /* the name of the format */
    const char *const *extensions;  /* without the dot, lower case */
    uint32_t extension_count;
    const char *const *mimes;       /* its MIME types, "image/x-portable-graymap": the host finds
                                       the plugin of some bytes by them */
    uint32_t mime_count;
} ww_format;

/* A file the plugin embeds: a page of its guide ("guide.md"), a sample, a translation of its texts
 * ("lang/uk/ui.json", a JSON object of the English texts and their translations). */
typedef struct ww_file {
    const char *path;               /* its path in the set, folders apart by "/" */
    const uint8_t *bytes;
    uint64_t size;
} ww_file;

/* Some bytes the host reads for the plugin: a file, or a part of one, read in blocks through the
 * cache of the host, or bytes in memory. */
typedef struct ww_source {
    uint32_t size;                  /* sizeof(ww_source) of the host */
    uint64_t length;                /* the bytes of the source */
    void *self;                     /* the host's own, given back to read */
    /* Copies at most count bytes from offset into out; gives how many (0 at the end or on an
     * error of the disk). */
    uint32_t (*read)(void *self, uint64_t offset, uint8_t *out, uint32_t count);
    const char *path;               /* the file the bytes come from, or NULL for bytes in memory */
    uint64_t offset;                /* where they start in that file */
} ww_source;

/* The state of the cache of the blocks of the host. */
typedef struct ww_cache {
    uint32_t size;                  /* sizeof(ww_cache) the plugin passes */
    uint32_t block;                 /* the bytes of a block */
    uint64_t budget;                /* the most bytes of blocks in memory */
    uint64_t used;                  /* the bytes of blocks in memory now */
    uint64_t blocks;                /* the blocks in memory now */
    uint64_t files;                 /* the files read through the cache that are open */
    uint64_t reads;                 /* the blocks read from the disk so far */
    uint64_t hits;                  /* the blocks found in memory so far */
} ww_cache;

/* What the host lends a plugin: its table, given to ww_plugin_get. */
typedef struct ww_host {
    uint32_t size;                  /* sizeof(ww_host) of the host */
    uint32_t api_version;
    /* A file read in blocks through the cache: a source, or NULL with the reason written into
     * error (error_size bytes). */
    const ww_source *(*open_file)(const char *path, char *error, uint32_t error_size);
    /* A part of a source as a source of its own, from offset and at most length bytes long,
     * reading the same blocks. */
    const ww_source *(*slice)(const ww_source *source, uint64_t offset, uint64_t length);
    /* Frees a source of open_file or slice; a source given to open_bytes is the host's. */
    void (*close_source)(const ww_source *source);
    /* Writes the state of the cache, the fields its size covers. */
    void (*cache)(ww_cache *out);
    /* The host's own, given back to call_tool: it names the plugin the table was given to. */
    void *self;
    /* Calls a tool of the program by its name, as the plugin: the switches, the mode and the
     * options of the server apply. args is a JSON object (NULL for none); the answer (a JSON
     * value, or the message of the error) is written into *answer and freed with free_text.
     * Gives WW_TOOL_OK or the kind of the error. */
    int32_t (*call_tool)(void *self, const char *name, const char *args, char **answer);
    void (*free_text)(char *text);
} ww_host;

typedef struct ww_key {
    const char *name;               /* the W3C name: "ArrowLeft", "PageUp", "Escape", "a", "+" */
    uint32_t pressed;               /* 1: pressed (or repeated while held) */
    uint32_t modifiers;             /* WW_SHIFT | WW_CTRL | WW_ALT */
} ww_key;

typedef struct ww_input {
    uint32_t size;                  /* sizeof(ww_input) of the host */
    uint32_t width, height;         /* the view, in pixels */
    float scale;                    /* pixels a point of the window */
    uint32_t focused, hovered;      /* 1 or 0 */
    float mouse_x, mouse_y;         /* the pointer in the pixels of the view, while over it or
                                       dragging */
    uint32_t buttons;               /* the buttons held now: WW_LEFT | WW_RIGHT | WW_MIDDLE */
    uint32_t clicks;                /* the buttons clicked since the last frame */
    uint32_t double_clicks;         /* the buttons clicked twice since the last frame */
    float drag_x, drag_y;           /* the pixels the pointer moved with a button down */
    float wheel_x, wheel_y;         /* lines of the wheel since the last frame, down positive */
    uint32_t modifiers;             /* held now */
    const ww_key *keys;             /* the keys of the view since the last frame */
    uint32_t key_count;
    const char *text;               /* text typed since the last frame (empty for now) */
} ww_input;

typedef struct ww_control {
    uint32_t id;                    /* the plugin's own number, given back to control() */
    uint32_t kind;                  /* WW_BUTTON, WW_TOGGLE, WW_SLIDER, WW_CHOICE, WW_LABEL */
    const char *label;
    double value, min, max, step;
    const char *const *choices;     /* the texts of a choice */
    uint32_t choice_count;
} ww_control;

typedef struct ww_output {
    uint32_t size;                  /* sizeof(ww_output) of the host; the rest the host set to
                                       zero */
    uint32_t width, height, stride; /* the picture: RGBA, 8 bits a channel, stride in bytes */
    const uint8_t *pixels;
    uint32_t changed;               /* 1: the picture differs from the last one */
    const ww_control *controls;     /* the controls of the bar of the view */
    uint32_t control_count;
    uint32_t keys;                  /* WW_KEYS_* the view takes while it has the focus */
    uint32_t cursor;                /* WW_CURSOR_* */
    uint32_t again;                 /* 1: draw another frame at once, the plugin is at work */
    const char *status;             /* a line of the status bar */
} ww_output;

typedef struct ww_plugin {
    uint32_t api_version;           /* WW_API_VERSION the plugin was written for */
    uint32_t size;                  /* sizeof(ww_plugin) of the plugin */
    const char *name;
    const ww_format *formats;
    uint32_t format_count;
    const ww_file *files;           /* the files it embeds, or NULL */
    uint32_t file_count;
    /* Opens a file: a view, or NULL with the reason written into error (error_size bytes). */
    void *(*open)(const char *path, char *error, uint32_t error_size);
    /* Opens some bytes under a name, as a MIME type of its formats (with its parameters,
     * "text/plain; charset=utf-16le"): a part of another file offered to the plugin, bytes
     * inside another view. The source lives until close of the view. NULL: files alone. */
    void *(*open_bytes)(const ww_source *source, const char *name, const char *mime, char *error,
                        uint32_t error_size);
    /* The MIME type of a format of the plugin the first bytes of a file show (at most 512), or
     * NULL: a text that lives for the run (one of the mimes of its formats). NULL: none. */
    const char *(*sniff)(const uint8_t *bytes, uint32_t count);
    /* A frame of the view: the input since the last one, the picture and the controls now. */
    void (*frame)(void *view, const ww_input *input, ww_output *output);
    /* A control of the bar changed: a button pressed, a toggle, a slider or a choice set. */
    void (*control)(void *view, uint32_t id, double value);
    /* The view closes; the plugin frees it. */
    void (*close)(void *view);
} ww_plugin;

/* A tool of a plugin. */
typedef struct ww_tool {
    const char *name;               /* letters, digits, "_", "-" and ".", unique in the program */
    const char *description;        /* for the agent: what it does and what it gives */
    const char *schema;             /* the JSON schema of its arguments, an object */
    const char *group;              /* the tools a switch of the tab turns off together */
    uint32_t mode;                  /* WW_MODE_* it needs */
    const char *needs;              /* the options it needs, "key=value,key=value", or NULL */
} ww_tool;

/* An option of the tools of a plugin: an access setting a tool needs, or a value a tool reads,
 * kept in the branch of the plugin of the configuration file of the server. */
typedef struct ww_option {
    const char *key;
    const char *title;              /* the name shown (English, translated by the plugin's files) */
    const char *const *values;      /* the values, the weakest first; NULL with 0: a text */
    const char *const *labels;      /* the names shown of the values, or NULL */
    uint32_t value_count;
    const char *initial;            /* the value without a setting */
    const char *help;               /* a line under it, or NULL */
} ww_option;

/* The tools of a plugin: its table, given by ww_tools_get. */
typedef struct ww_tools {
    uint32_t api_version;           /* WW_API_VERSION */
    uint32_t size;                  /* sizeof(ww_tools) of the plugin */
    const char *name;               /* the name of the source of the tools */
    const ww_tool *tools;
    uint32_t tool_count;
    const ww_option *options;
    uint32_t option_count;
    /* Runs a tool after the host checked the switches, the mode and the options: its arguments
     * and the values of the options of the plugin as JSON objects. Writes the answer (a JSON
     * value, or the message of the error) into *answer, which the host frees with free_answer;
     * gives WW_TOOL_OK or the kind of the error. */
    int32_t (*call)(const char *tool, const char *args, const char *options, char **answer);
    void (*free_answer)(char *answer);
} ww_tools;

#ifdef _WIN32
#define WW_EXPORT __declspec(dllexport)
#else
#define WW_EXPORT __attribute__((visibility("default")))
#endif

/* The views of a plugin: the host gives its table, which lives for the run. */
WW_EXPORT const ww_plugin *ww_plugin_get(const ww_host *host);

/* The tools of a plugin, a function of its own: a DLL may export it alone (tools without a
 * view), ww_plugin_get alone, or both. The host gives its table, which lives for the run. */
WW_EXPORT const ww_tools *ww_tools_get(const ww_host *host);

#ifdef __cplusplus
}
#endif

#endif /* WW_H */
