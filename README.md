## install

### `cargo build --release`

### `sudo nano  ~/Library/LaunchAgents/com.github.acsandmann.apple-music-rpc.plist`

```xml
<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple/DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>com.github.acsandmann.apple-music-rpc</string>
    <key>ProgramArguments</key>
    <array>
        <string>path to release build (for example, /Users/bob/Downloads/apple-music-rpc/target/release/apple-music-rpc</string>
    </array>
    <key>RunAtLoad</key>
    <true/>
    <key>KeepAlive</key>
    <true/>
</dict>
</plist>
```

### `launchctl load ~/Library/LaunchAgents/com.github.acsandmann.apple-music-rpc.plist`

#### if you are still having issues i recommend `brew install --cask launchcontrol`. it is a launch control gui and whilst it is paid you can surely use the free trial to figure some stuff out.