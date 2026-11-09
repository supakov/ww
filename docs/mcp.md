# The MCP server of W#: `wwmcp`

A program on W# serves its tools and the tools of its plugins to AI agents over MCP (the protocol
2025-06-18 on JSON-RPC 2.0), and calls any of them by its name itself. The crate `wwmcp` holds the
server, its configuration file, its journal and log, the transports and the command line; with
its feature `ui` it draws the tab of the server in a window of W#. `wwrt` takes it with the
feature and makes the plugins sources of tools; `wwrt::host` gives a program with plugins the tab,
the tools of its window and a start without the window.

## 1. Sources and tools

A source of tools (`wwmcp::Source`) is the program or a plugin. It names:

| What | Where |
|---|---|
| its id, the key of its branch of the configuration | `Source::id`: the name of the program, or of the DLL of a plugin |
| its tools | `ToolDef`: a name (letters, digits, `_`, `-`, `.`), a description for the agent, the JSON schema of its arguments, a group, the mode it needs, the options it needs |
| its options | `Choice`: a choice of named values, the weakest first (`off`, `read`, `write`), or a text; a title and a help line for the tab |
| its resources | `ResourceDef` and `Source::read` |
| what it tells the client first | `Source::instructions` |
| the fields whose values never reach the journal | `Source::secrets`, beyond `password`, `token`, `authorization` and `secret` |

The modes are `read` (nothing changes), `edit` (what the program holds changes in memory) and
`files` (files are written too); the server runs in one of them, and a tool of a higher mode is
refused. An option is an access setting a tool needs (`ToolDef::needs("registry", "read")`: the
option at that value or above) or a value a tool reads (`Call::option`). A source keeps its state
safe for several threads: the calls come from the threads of the transports and of the window, a
tool may call another one by its name, and the server holds no lock while a tool runs.

## 2. The server and a call by name

`Server::new(Info, Config).with(source)` makes a server; `Server::tools` lists every tool with
its source and whether it is on (the first of a name; a later one of the same name is left out),
`Server::handle_text` answers a message of JSON-RPC, and `Server::call(name, args, caller)` runs a
tool by its name. A caller other than the program (`Caller::Client`, `Caller::Plugin(id)`) meets
the switches of the configuration (a tool, a group or a source off is not found), the mode and
the options the tool needs; the program (`Caller::Program`: a command, a script, the call by hand
of the tab) calls what its user asked for. Every call goes into the journal (the last 500, the
arguments that are identifiers, numbers or switches shown, the whole arguments and the answer
without the secret fields) and into the log.

This is the door of a scripting language to come: a script calls the tools of the program and of
its plugins by their names, as an agent does.

## 3. The configuration file `.wwmcp`

`mcp/default.wwmcp` next to the executable, or a file named on the command line; the tab saves it
at every change and writes no file until the first one.

```json
{
  "format": "ww-mcp",
  "version": 1,
  "transport": "http",
  "port": 8765,
  "token": "<40 letters and digits>",
  "mode": "read",
  "detailed_log": false,
  "plugins": {
    "ww_hello": {
      "groups_off": ["hello"],
      "options": { "greeting": "warm", "open_tabs": "on" }
    },
    "ww_mandelbrot": { "off": true },
    "window": { "tools_off": ["save_file"] }
  }
}
```

The branch `plugins` holds the settings of every source by its id, the program among them: `off`
for the whole source, `tools_off` and `groups_off` by name, `options` by key (a value that is not
one of the option's is taken as its default; an option not named has its default). A source the
file does not name has everything on. The file is read strictly: an unknown field or a wrong
value is an error, and a file that cannot be read is not written over.

## 4. Transports and the command line

- HTTP on 127.0.0.1: `POST /mcp` with `Authorization: Bearer <token>` (32 letters and digits or
  more), the Host and the Origin of the loopback only (no web page reaches it), 8 connections,
  4 MB a message. The tab starts it with the window.
- stdio: a message a line, for a client that starts the program itself.

`wwmcp::cli` starts a server without the window: `PROGRAM --mcp FILE|stdio|http`, with
`--config FILE`, `--mode`, `--port`, `--token`, `--detailed-log`, `--off SOURCE[/TOOL]`,
`--set SOURCE.KEY=VALUE`, and `--KEY VALUE` (or `--KEY` alone for a switch `off`/`on`) for the
option of the source that has it, dashes for underscores. `wwmcp::cli::USAGE` gives the lines for
the help of a program. `wwrt::host::serve_mcp` does it for a program with plugins.

## 5. The log

`wwmcp::log::init(dir, program, headless)` sets the folder of the logs and the panic hook. The
server writes `mcp-<date>.jsonl`, a JSON object a line: the start and the stop of a transport,
every call with its result and its time, the refusals; with `detailed_log` every call and answer
whole (`level` DETAIL, the secret fields left out). A panic of a thread of the server or of a
process that only serves MCP goes there too; `log::fatal` writes an exception of the system the
program catches. The tab has **Open the log**, which opens it as a file of the program: in a host
of plugins, a plugin of JSON Lines shows it as the list of its lines.

## 6. The tab (feature `ui`)

`wwmcp::ui::McpPanel` is the state of the tab (the configuration, the server, the HTTP transport,
the journal selected, the call by hand) and `wwmcp::ui::draw` draws it: the settings above (the
server: the file, the state, the port, the mode, the log, the transport of a start without the
window, the token and the configurations for clients; the cards of the program; **Tools**: each
source with its switch, its options, its groups and its tools, each with its switch; **Call a
tool**: a tool, its arguments as JSON filled from its schema, and the answer), the journal below
and the details of the call selected at the right. `McpPanel::apply` answers its acts but
`OpenLog`, which is the program's; `McpPanel::persist` writes the file once it changed. The
texts of the tab are English sources of `tr` (`wwmcp::SOURCES` for the tests of translations);
the names a source gives (its title, its options, its groups) are translated at run time
(`ww::i18n::tr_any`) by the translations of the program or of the plugin.

## 7. The tools of the window

`wwmcp::window::WindowTools` is the source `window` of a program on W#: `list_files` (read: the
tabs, their titles and files, whether they have changes), `open_file` (edit: a file in a tab, by
the program), `close_file` (edit: a tab by its number from 1 or its title; one with changes stays
unless `discard` is true) and `save_file` (files: a tab that has a file). `ProgramTools` adds the
program's own tools that act on its state (the demo: `new_note`). A call comes on a thread of the
server and waits while the window answers it at its next frame (`wwmcp::window::answer`, which a
program calls in `App::draw`; `wwrt::host::draw` does it); the window wakes for it. A call from
the thread of the window itself is refused, so the call by hand of the tab runs on a thread of
its own.

## 8. A program with plugins (`wwrt::host`)

`HostMcp::new(info, plugins, program_tools, own_sources)` makes the server of a program with
plugins: the tools of its window, its own tools (answered by `PluginApp::program_tool`), its
other sources and the ones of its plugins, the configuration `mcp/default.wwmcp`, and installs
it for the calls of the plugins (`wwrt::tools::install`). The program returns it from
`PluginApp::mcp` and `mcp_mut`; the host draws the tab (`HostDoc::Mcp`, the item **MCP server**
of the menu of the plugins, `HostAct::OpenMcp`), answers its acts (`HostAct::Mcp`) and the tools
of the window at every frame. `serve_mcp` is `--mcp` of the program, without the window and its
tools. The demo (`examples/ww-demo`) does all of it.

## 9. Tools of a plugin

- A native plugin (docs/plugins.md, section 10): `Plugin::tools`, `Plugin::tool_options` and
  `Plugin::call_tool`; a plugin with tools and no formats is a source of tools without a view.
  `Host::call_tool` (in a view) and `wwrt::tools::call(name, args, id)` call a tool of the
  program by its name, as the plugin.
- A plugin of the stable interface (docs/stable.md, section 2a): the function `ww_tools_get`,
  with or without `ww_plugin_get`, gives a table `ww_tools` (the tools, the options, `call` and
  `free_answer`); `call_tool` of `ww_host` calls a tool of the program by its name. The crate
  `ww-stable` writes the table with `tools!` and calls with `ww_stable::call_tool`.

The examples: `ww-hello` (a greeting that reads its option, a count of the words, its sample
opened in a tab through the tool `open_file` of the program, which its access option allows),
`ww-mandelbrot` in C (how fast a point leaves the set, the set drawn in letters, its sample
opened through `call_tool`; the option `limit` caps the iterations) and `ww-grey` in stable Rust
(the state of the cache of the host).

## 10. Safety

Nothing reaches the server but the loopback with the token, or the process that started the
program (stdio). Every start is in the mode of its configuration (`read` by default); the user
turns tools and sources off and sets the options in the tab. Passwords, tokens and the secret
fields of the sources never reach the journal or the log; the configurations copied for clients
hold the token, and the clipboard history of Windows keeps what is copied.
