# Hello, the template of a plugin

The template of a native plugin of a program on W#. It opens `.hello` files, and any text whose
first line starts with `Hello`, and shows that first line large in the middle of its tab.

- The wheel, `+` and `-` change the size of the text; a drag and the arrows move it; a double
  click and `0` put it back.
- Reset and Frame are in the bar of the tab (Frame is a switch, drawn pressed while it is on);
  Bigger and Smaller are in the menu of the window; Say hello is in the submenu of the plugin.
- The right button opens the context menu of the view: its items and Open with, which opens the
  text of the hello with another plugin.
- Its setting, the colour of the text, is in Settings, Plugins.

The plugin embeds this page, [a sample](../samples/sample.hello) and the translations of its
texts. A link to a file of the set opens it in a tab, by the plugin of its type: here by this
plugin itself.
