# Test plan

1. Local configuration: invalid/unknown versions, clamped dimensions and atomic save.
2. Database: snapshot with WAL content; validation rejection; same-folder lock;
   staged restoration and restart recovery at rename boundaries.
3. Data operations: purpose/collection-scoped retention and portable destination
   collision, copied accounts/blobs/settings and disabled services.
4. Installer: PowerShell parse and Inno compilation when tools are available;
   static contract checks plus manual x64/ARM64 installation matrix.
5. Run focused tests during fixes, then workspace fmt/clippy/test/deny.
6. Update channels: test explicit opt-in from stable, explicit opt-out from beta,
   promoted beta on Latest, unsigned candidate rejection, no downgrade after
   switching back, and settings default/unknown values preserving other fields.
