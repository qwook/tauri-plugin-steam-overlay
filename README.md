# Tauri Plugin steam-overlay

This plugin creates a window overlay with a surface that floats above the main window. Steam can detect and draw on this overlay.

_You must add the compiled app to Steam and launch from Steam in order to see the overlay._

https://github.com/user-attachments/assets/cff8a150-8320-4fc2-9b87-27b5757e50be

If it's not already handled in your build pipeline, you may need to download and copy the [steamworks redistributable](https://partner.steamgames.com/doc/sdk) next to your executable.

# Future?

Maybe I'll turn this into full-blown steam integration with JS library and build step that copies the redistributables if there's enough interest.
