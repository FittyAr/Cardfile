# Verifying Cardfile downloads

Every file attached to a [release](https://github.com/FittyAr/Cardfile/releases) comes with a
[minisign](https://jedisct1.github.io/minisign/) signature (`<file>.minisig`) and is listed in `SHA256SUMS`.

## Public key

```
RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w
```

The same key is built into Cardfile (`cardfile-updater`): the app only announces a new version
when that release's `SHA256SUMS` is signed with it.

## Verify a file

```bash
minisign -Vm Cardfile-2.0.0-setup-x64.exe -P RWSEWpMd7tkiGAaQwVFXiSYyfV6ODYRttbEFQ/WBOTzrbMB1diAfrG+w
```

Or check the checksum:

```bash
sha256sum -c SHA256SUMS --ignore-missing
```

```powershell
Get-FileHash Cardfile-2.0.0-setup-x64.exe -Algorithm SHA256
```

## About code signing on Windows

The Microsoft Store package is signed by Microsoft. The `.exe` installer and the archives are not
Authenticode-signed (that needs a paid certificate), so Windows SmartScreen may show a warning the
first time you run them. You can check them with minisign as shown above, install from the
Microsoft Store or winget, or build from source.
