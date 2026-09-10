namespace Wayscriber.Tools;

internal static class NativeDesktopCommands
{
    private const uint RootUserId = 0;
    private const int DownloadTimeoutSeconds = 60;
    private const int DaemonRestartDelayMilliseconds = 500;
    private const string GlsVersion = "1.3.0";
    private const string GlsCommit = "1c963c51514581c41b9bdae08cdf69171265cdda";
    private const string GlsArchiveSha256 = "22be5f5edf487cfb87266f0e71c400b11322082a4dc99832e5a54a4fca3d5a7c";
    private static readonly int[] ConfiguratorIconSizes = [16, 19, 22, 24, 38, 64, 128];

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Elf, CommandNames.VerifyStatic, "Verify static gtk4-layer-shell linkage.", SideEffect.ReadOnly, VerifyStatic ),
        new( CommandAreas.Elf, CommandNames.VerifyDynamic, "Verify dynamic gtk4-layer-shell linkage.", SideEffect.ReadOnly, VerifyDynamic ),
        new( CommandAreas.Native, CommandNames.InstallGtk4LayerShell, "Build the pinned gtk4-layer-shell source.", SideEffect.MachineMutating, InstallGtkLayerShell ),
        new( CommandAreas.Desktop, CommandNames.SetPortalShortcut, "Set the portal shortcut service environment.", SideEffect.MachineMutating, SetPortalShortcut ),
        new( CommandAreas.Desktop, CommandNames.ReloadDaemon, "Restart the Wayscriber daemon.", SideEffect.MachineMutating, ReloadDaemon ),
        new( CommandAreas.Install, CommandNames.App, "Build and install Wayscriber locally.", SideEffect.MachineMutating, InstallApp ),
        new( CommandAreas.Install, CommandNames.Configurator, "Build and install the configurator locally.", SideEffect.MachineMutating, InstallConfigurator ),
    ];

    private static async Task<int> VerifyStatic( ToolContext context, string[] args )
    {
        var binary = Path.GetFullPath( new Arguments( args ).SinglePositional( "elf verify-static BINARY" ), Environment.CurrentDirectory );
        if ( !File.Exists( binary ) )
        {
            throw new ToolException( $"Missing Wayscriber binary: {binary}" );
        }
        var dynamic = await context.Run( Programs.ReadElf, ["-d", binary], capture: true );
        if ( dynamic.StandardOutput.Contains( "Shared library: [libgtk4-layer-shell.so", StringComparison.Ordinal ) )
        {
            throw new ToolException( $"{binary} still dynamically requires gtk4-layer-shell" );
        }
        if ( !dynamic.StandardOutput.Contains( "Shared library: [libwayland-client.so.0]", StringComparison.Ordinal ) )
        {
            throw new ToolException( $"{binary} does not retain libwayland-client.so.0 in DT_NEEDED" );
        }
        var symbols = await context.Run( Programs.Nm, ["-D", "--defined-only", binary], capture: true );
        var names = symbols.StandardOutput.Split( '\n', StringSplitOptions.RemoveEmptyEntries )
            .Select( line => line.Split( ' ', StringSplitOptions.RemoveEmptyEntries ).LastOrDefault( ) ).ToHashSet( StringComparer.Ordinal );
        foreach ( var symbol in new[] { "wl_proxy_destroy", "wl_proxy_marshal_array_flags", "wl_proxy_marshal_flags", "wl_proxy_marshal",
                     "wl_proxy_marshal_array", "wl_proxy_marshal_constructor", "wl_proxy_marshal_constructor_versioned",
                     "wl_proxy_marshal_array_constructor", "wl_proxy_marshal_array_constructor_versioned" } )
        {
            if ( !names.Contains( symbol ) )
            {
                throw new ToolException( $"{binary} does not export required gtk4-layer-shell shim {symbol}" );
            }
        }
        await context.Output.WriteLineAsync( $"Verified static gtk4-layer-shell linkage and exported Wayland shims in {binary}" );
        return ExitCodes.Success;
    }

    private static async Task<int> VerifyDynamic( ToolContext context, string[] args )
    {
        var binary = Path.GetFullPath( new Arguments( args ).SinglePositional( "elf verify-dynamic BINARY" ), Environment.CurrentDirectory );
        if ( !File.Exists( binary ) )
        {
            throw new ToolException( $"Missing Wayscriber binary: {binary}" );
        }
        var dynamic = await context.Run( Programs.ReadElf, ["-d", binary], capture: true );
        if ( !dynamic.StandardOutput.Contains( "Shared library: [libgtk4-layer-shell.so", StringComparison.Ordinal ) )
        {
            throw new ToolException( $"{binary} does not dynamically require gtk4-layer-shell" );
        }
        await context.Output.WriteLineAsync( $"Verified dynamic gtk4-layer-shell linkage in {binary}" );
        return ExitCodes.Success;
    }

    private static async Task<int> InstallGtkLayerShell( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var prefix = parsed.TakeOption( "--prefix" ) ?? context.Environment( EnvironmentVariables.Gtk4LayerShellPrefix ) ?? "/usr";
        var systemPrefix = parsed.TakeOption( "--system-prefix" ) ?? context.Environment( EnvironmentVariables.Gtk4LayerShellSystemPrefix ) ?? "/usr";
        var mode = parsed.TakeOption( "--library-mode" ) ?? context.Environment( EnvironmentVariables.Gtk4LayerShellLibraryMode ) ?? NativeLibraryModes.Shared;
        var force = parsed.TakeFlag( "--force" ) || context.Environment( EnvironmentVariables.ForceBuild ) == EnvironmentVariables.Enabled;
        parsed.RequireEmpty( "native install-gtk4-layer-shell [--prefix PATH] [--library-mode shared|static|both] [--force]" );
        if ( !Path.IsPathFullyQualified( prefix ) || !Path.IsPathFullyQualified( systemPrefix ) )
        {
            throw new ToolException( "gtk4-layer-shell prefixes must be absolute." );
        }
        if ( mode is not (NativeLibraryModes.Shared or NativeLibraryModes.Static or NativeLibraryModes.Both) )
        {
            throw new ToolException( "library mode must be shared, static, or both." );
        }
        var pin = $"version={GlsVersion}\ncommit={GlsCommit}\narchive_sha256={GlsArchiveSha256}\nlibrary_mode={mode}\n";
        var stamp = Path.Combine( prefix, "share/wayscriber/build-deps/gtk4-layer-shell.pin" );
        var pkgEnvironment = new Dictionary<string, string?>
        {
            [EnvironmentVariables.PackageConfigPath] = Path.Combine( prefix, "lib/pkgconfig" ) + (context.Environment( EnvironmentVariables.PackageConfigPath ) is { Length: > 0 } existing ? $":{existing}" : string.Empty),
        };
        if ( !force && await GtkArtifactsExist( context, prefix, systemPrefix, mode, stamp, pin, pkgEnvironment ) )
        {
            var version = await context.Run( Programs.PkgConfig, ["--modversion", "gtk4-layer-shell-0"], environment: pkgEnvironment, capture: true );
            await context.Output.WriteLineAsync( $"[install-gtk4-layer-shell] gtk4-layer-shell {version.StandardOutput.Trim( )} ({mode}) already available in {prefix}; skipping build" );
            return ExitCodes.Success;
        }
        using var temporary = new TemporaryDirectory( RepositoryNames.Gtk4LayerShell );
        var archive = Path.Combine( temporary.Path, "source.tar.gz" );
        await context.Output.WriteLineAsync( $"[install-gtk4-layer-shell] Downloading gtk4-layer-shell {GlsVersion} (commit {GlsCommit})" );
        using ( var client = new HttpClient { Timeout = TimeSpan.FromSeconds( DownloadTimeoutSeconds ) } )
        using ( var response = await client.GetAsync( $"https://github.com/wmww/gtk4-layer-shell/archive/{GlsCommit}.tar.gz", HttpCompletionOption.ResponseHeadersRead, context.CancellationToken ) )
        {
            response.EnsureSuccessStatusCode( );
            await using var input = await response.Content.ReadAsStreamAsync( context.CancellationToken );
            await using var output = File.Create( archive );
            await input.CopyToAsync( output, context.CancellationToken );
        }
        if ( Files.Sha256( archive ) != GlsArchiveSha256 )
        {
            throw new ToolException( "gtk4-layer-shell archive checksum mismatch." );
        }
        await context.Run( Programs.Tar, ["-xzf", archive, CommandLineOptions.ChangeDirectory, temporary.Path] );
        var source = Path.Combine( temporary.Path, $"gtk4-layer-shell-{GlsCommit}" );
        if ( !File.ReadAllBytes( Path.Combine( source, "LICENSE" ) ).SequenceEqual( File.ReadAllBytes( context.Path( RepositoryPaths.PackagingDirectory, "licenses", "gtk4-layer-shell.LICENSE" ) ) ) )
        {
            throw new ToolException( "Pinned gtk4-layer-shell LICENSE differs from the tracked notice." );
        }
        var build = Path.Combine( source, "_build" );
        await context.Run( Programs.Meson, ["setup", build, source, $"--prefix={prefix}", "--libdir=lib", "--buildtype=release", $"--default-library={mode}",
            "-Dexamples=false", "-Ddocs=false", "-Dtests=false", "-Dintrospection=false", "-Dvapi=false"] );
        await context.Run( Programs.Meson, ["compile", CommandLineOptions.ChangeDirectory, build] );
        var privileged = prefix == systemPrefix || prefix.StartsWith( systemPrefix.TrimEnd( '/' ) + "/", StringComparison.Ordinal );
        await RunPossiblyRoot( context, privileged, Programs.Meson, ["install", CommandLineOptions.ChangeDirectory, build] );
        var pinFile = Path.Combine( temporary.Path, "gtk4-layer-shell.pin" );
        Files.WriteAtomic( pinFile, pin );
        await RunPossiblyRoot( context, privileged, Programs.Install, [InstallArguments.DataFile, pinFile, stamp] );
        if ( privileged )
        {
            await RunPossiblyRoot( context, true, Programs.LdConfig, [], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        }
        if ( !await GtkArtifactsExist( context, prefix, systemPrefix, mode, stamp, pin, pkgEnvironment ) )
        {
            throw new ToolException( $"{mode} gtk4-layer-shell artifacts unavailable after install." );
        }
        await context.Output.WriteLineAsync( $"[install-gtk4-layer-shell] Installed gtk4-layer-shell {GlsVersion} ({mode}) to {prefix}" );
        return ExitCodes.Success;
    }

    internal static async Task<bool> GtkArtifactsExist( ToolContext context, string prefix, string systemPrefix, string mode, string stamp, string pin,
        IReadOnlyDictionary<string, string?> environment )
    {
        if ( File.Exists( stamp ) )
        {
            if ( Files.Read( stamp ) != pin )
            {
                return false;
            }
        }
        else if ( prefix != systemPrefix )
        {
            return false;
        }

        var probe = await context.Run( Programs.PkgConfig, [$"--atleast-version={GlsVersion}", "gtk4-layer-shell-0"], environment: environment,
            capture: true, trace: false, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( probe.ExitCode != ExitCodes.Success )
        {
            return false;
        }

        var dirResult = await context.Run( Programs.PkgConfig, ["--variable=libdir", "gtk4-layer-shell-0"], environment: environment, capture: true, trace: false );
        var directory = dirResult.StandardOutput.Trim( );
        if ( !(directory == prefix || directory.StartsWith( prefix.TrimEnd( '/' ) + "/", StringComparison.Ordinal )) )
        {
            return false;
        }

        var shared = File.Exists( Path.Combine( directory, "libgtk4-layer-shell.so" ) ) || File.Exists( Path.Combine( directory, "libgtk4-layer-shell.so.0" ) );
        var @static = File.Exists( Path.Combine( directory, "libgtk4-layer-shell.a" ) );
        return mode switch
        {
            NativeLibraryModes.Shared => shared,
            NativeLibraryModes.Static => @static,
            _ => shared && @static
        };
    }

    private static async Task<int> SetPortalShortcut( ToolContext context, string[] args )
    {
        var shortcut = args.Length > 0 ? args[0] : "<Ctrl><Shift>g";
        var appId = args.Length > 1 ? args[1] : RepositoryNames.MainPackage;
        if ( args.Length > 2 )
        {
            throw new ToolException( "desktop set-portal-shortcut [SHORTCUT] [APP_ID]", ExitCodes.InvalidArguments );
        }
        var home = context.Environment( EnvironmentVariables.Home );
        if ( home is null )
        {
            throw new ToolException( ToolMessages.HomeNotSet );
        }

        var path = Path.Combine( home, ".config/systemd/user/wayscriber.service.d/shortcut.conf" );
        string Escape( string value ) => value.Replace( "\\", "\\\\", StringComparison.Ordinal ).Replace( "\"", "\\\"", StringComparison.Ordinal );
        Files.WriteAtomic( path, $"[Service]\nEnvironment=\"WAYSCRIBER_PORTAL_SHORTCUT={Escape( shortcut )}\"\nEnvironment=\"WAYSCRIBER_PORTAL_APP_ID={Escape( appId )}\"\n" );
        await context.Run( Programs.SystemControl, ["--user", "daemon-reload"] );
        await context.Run( Programs.SystemControl, ["--user", "restart", RepositoryNames.UserServiceFile] );
        await context.Output.WriteLineAsync( $"Updated {path}\nShortcut set to: {shortcut}\nPortal app id set to: {appId}\nwayscriber.service restarted." );
        return ExitCodes.Success;
    }

    private static async Task<int> ReloadDaemon( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "desktop reload-daemon" );
        await context.Output.WriteLineAsync( "Stopping wayscriber daemon..." );
        await context.Run( Programs.Pkill, [RepositoryNames.MainPackage], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        await Task.Delay( DaemonRestartDelayMilliseconds, context.CancellationToken );
        await context.Output.WriteLineAsync( "Starting wayscriber daemon..." );
        _ = System.Diagnostics.Process.Start( new System.Diagnostics.ProcessStartInfo( Programs.Wayscriber ) { UseShellExecute = false, ArgumentList = { "--daemon" } } );
        await Task.Delay( DaemonRestartDelayMilliseconds, context.CancellationToken );
        var probe = await context.Run( Programs.Pgrep, ["-x", RepositoryNames.MainPackage], capture: true, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( probe.ExitCode != ExitCodes.Success )
        {
            throw new ToolException( "Failed to start daemon." );
        }
        await context.Output.WriteLineAsync( $"Daemon restarted successfully (PID: {probe.StandardOutput.Trim( )})\nPress Super+D to toggle overlay" );
        return ExitCodes.Success;
    }

    private static async Task<int> InstallApp( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var replaceOther = parsed.TakeFlag( "--replace-other" );
        var skipAutostart = parsed.TakeFlag( "--skip-autostart" );
        var addHyprlandKeybind = parsed.TakeFlag( "--add-hyprland-keybind" );
        var autostart = parsed.TakeOption( "--autostart" );
        var installDir = parsed.TakeOption( "--install-dir" ) ?? context.Environment( EnvironmentVariables.WayscriberInstallDirectory ) ?? "/usr/bin";
        parsed.RequireEmpty( "install app [--install-dir PATH] [--replace-other] [--skip-autostart] [--autostart systemd|hyprland|none] [--add-hyprland-keybind]" );
        ValidateInstallOptions( skipAutostart, autostart );

        installDir = NormalizeBinDirectory( installDir );
        var destination = Path.Combine( installDir, RepositoryNames.MainPackage );
        var home = context.Environment( EnvironmentVariables.Home );
        if ( home is null )
        {
            throw new ToolException( ToolMessages.HomeNotSet );
        }

        var systemdUser = Path.Combine( context.Environment( EnvironmentVariables.XdgConfigHome ) ?? Path.Combine( home, ".config" ), "systemd/user" );
        var conflicts = FindInstallConflicts( home, systemdUser, destination );
        await ConfirmInstallConflicts( context, conflicts, destination, replaceOther );
        await context.Run( Programs.Cargo, ["build", CommandLineOptions.Release, CommandLineOptions.Binaries] );
        await RemoveInstallConflicts( context, conflicts );

        await RunPossiblyRoot( context, NeedsPrivilege( installDir ), Programs.Install, ["-d", installDir] );
        await RunPossiblyRoot( context, NeedsPrivilege( installDir ), Programs.Install,
            [InstallArguments.ExecutableFile, context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.MainPackage ), destination] );
        var configDir = Path.Combine( home, ".config/wayscriber" );
        Directory.CreateDirectory( configDir );
        var config = Path.Combine( configDir, "config.toml" );
        if ( !File.Exists( config ) )
        {
            File.Copy( context.Path( "config.example.toml" ), config );
        }
        autostart = await SelectAutostart( context, skipAutostart, autostart );
        if ( autostart == AutostartModes.Systemd )
        {
            await InstallSystemdService( context, destination, installDir, systemdUser );
        }
        else if ( autostart == AutostartModes.Hyprland )
        {
            ConfigureHyprland( home, destination, includeAutostart: true );
        }
        if ( autostart == AutostartModes.Systemd && addHyprlandKeybind )
        {
            ConfigureHyprland( home, destination, includeAutostart: false );
        }
        await context.Output.WriteLineAsync( $"Installed: {destination}\nSHA256: {Files.Sha256( destination )}" );
        return ExitCodes.Success;
    }

    private static void ValidateInstallOptions( bool skipAutostart, string? autostart )
    {
        if ( skipAutostart && autostart is not null )
        {
            throw new ToolException( "--skip-autostart and --autostart cannot be combined.", ExitCodes.InvalidArguments );
        }
        if ( autostart is not null && autostart is not (AutostartModes.Systemd or AutostartModes.Hyprland or AutostartModes.None) )
        {
            throw new ToolException( "--autostart must be systemd, hyprland, or none.", ExitCodes.InvalidArguments );
        }
    }

    private static string[] FindInstallConflicts( string home, string systemdUser, string destination )
    {
        var known = new[] { "/usr/bin/wayscriber", "/usr/local/bin/wayscriber", Path.Combine( home, ".local/bin/wayscriber" ) };
        var overrideDirectory = Path.Combine( systemdUser, "wayscriber.service.d" );
        var userUnits = new[] { Path.Combine( systemdUser, RepositoryNames.UserServiceFile ) }
            .Concat( Directory.Exists( overrideDirectory ) ? Directory.EnumerateFiles( overrideDirectory, "*.conf" ) : [] );
        var systemUnits = new[] { "/usr/lib/systemd/user/wayscriber.service", "/usr/local/lib/systemd/user/wayscriber.service" };
        var binaries = known.Where( path => path != destination && (File.Exists( path ) || Directory.Exists( path )) && !SameFile( path, destination ) );
        var units = systemUnits.Concat( userUnits ).Where( path => HasConflictingExecStart( path, destination, known ) );
        return binaries.Concat( units ).Distinct( ).ToArray( );
    }

    private static async Task ConfirmInstallConflicts( ToolContext context, string[] conflicts, string destination, bool replaceOther )
    {
        if ( conflicts.Length == 0 || replaceOther )
        {
            return;
        }
        if ( Console.IsInputRedirected )
        {
            throw new ToolException( $"Another Wayscriber install exists ({string.Join( ", ", conflicts )}); pass --replace-other to replace it." );
        }

        await context.Output.WriteLineAsync( $"Another Wayscriber install exists:\n{string.Join( '\n', conflicts.Select( path => $"  {path}" ) )}" );
        await context.Output.WriteAsync( $"Remove those files so only {destination} remains? [y/N] " );
        var response = Console.ReadLine( );
        var confirmed = response is not null && (response.Equals( "y", StringComparison.OrdinalIgnoreCase ) || response.Equals( "yes", StringComparison.OrdinalIgnoreCase ));
        if ( !confirmed )
        {
            throw new ToolException( "Refusing to install beside the other copy. Re-run with --replace-other after removing it." );
        }
    }

    private static async Task RemoveInstallConflicts( ToolContext context, string[] conflicts )
    {
        foreach ( var conflict in conflicts )
        {
            if ( await IsPackageOwned( context, conflict ) )
            {
                if ( Path.GetFileName( conflict ) == RepositoryNames.MainPackage )
                {
                    throw new ToolException( $"{conflict} is package-owned; remove or update that package instead." );
                }
                await context.Error.WriteLineAsync( $"Leaving package-owned {conflict}." );
                continue;
            }

            await RunPossiblyRoot( context, NeedsPrivilege( conflict ), Programs.Remove, ["-f", CommandLineOptions.EndOfOptions, conflict] );
        }
    }

    private static async Task<string> SelectAutostart( ToolContext context, bool skipAutostart, string? autostart )
    {
        if ( skipAutostart || autostart is not null )
        {
            return autostart ?? AutostartModes.None;
        }
        if ( Console.IsInputRedirected )
        {
            throw new ToolException( "Choose --autostart systemd|hyprland|none for a non-interactive install." );
        }

        await context.Output.WriteAsync( "Autostart method: 1) systemd  2) Hyprland  3) none [1-3]: " );
        return Console.ReadLine( ) switch
        {
            "1" => AutostartModes.Systemd,
            "2" => AutostartModes.Hyprland,
            _ => AutostartModes.None
        };
    }

    private static async Task<int> InstallConfigurator( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var installDir = parsed.TakeOption( "--install-dir" ) ?? context.Environment( EnvironmentVariables.WayscriberInstallDirectory ) ?? "/usr/bin";
        var dataDir = parsed.TakeOption( "--data-dir" ) ?? context.Environment( EnvironmentVariables.WayscriberDataDirectory );
        parsed.RequireEmpty( "install configurator [--install-dir PATH] [--data-dir PATH]" );
        installDir = Path.GetFullPath( installDir );
        if ( dataDir is null )
        {
            if ( Path.GetFileName( installDir ) != "bin" )
            {
                throw new ToolException( $"Cannot derive data directory from {installDir}; pass --data-dir." );
            }
            dataDir = Path.Combine( Path.GetDirectoryName( installDir )!, "share" );
        }
        dataDir = Path.GetFullPath( dataDir );
        await context.Run( Programs.Cargo, ["build", CommandLineOptions.Release, CommandLineOptions.Binaries, "--manifest-path", RepositoryPaths.ConfiguratorCargoManifest] );
        var binary = new[] { context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.ConfiguratorPackage ),
            context.Path( RepositoryPaths.ConfiguratorDirectory, RepositoryPaths.TargetDirectory, RepositoryPaths.ReleaseDirectory, RepositoryNames.ConfiguratorPackage ) }
            .FirstOrDefault( File.Exists );
        if ( binary is null )
        {
            throw new ToolException( "Configurator binary not found after build." );
        }

        var destination = Path.Combine( installDir, RepositoryNames.ConfiguratorPackage );
        await RunPossiblyRoot( context, NeedsPrivilege( installDir ), Programs.Install, [InstallArguments.ExecutableFile, binary, destination] );
        var desktop = Files.Read( context.Path( RepositoryPaths.PackagingDirectory, "wayscriber-configurator.desktop" ) );
        var escaped = EscapeDesktopExecPath( destination );
        desktop = System.Text.RegularExpressions.Regex.Replace( desktop, @"(?m)^Exec=.*$", $"Exec=\"{escaped}\"" );
        desktop = System.Text.RegularExpressions.Regex.Replace( desktop, @"(?m)^TryExec=.*$", $"TryExec={destination.Replace( "\\", "\\\\", StringComparison.Ordinal )}" );
        using var temporary = new TemporaryDirectory( "wayscriber-desktop" );
        var desktopFile = Path.Combine( temporary.Path, "wayscriber-configurator.desktop" );
        Files.WriteAtomic( desktopFile, desktop );
        await InstallData( context, desktopFile, Path.Combine( dataDir, "applications/wayscriber-configurator.desktop" ) );
        foreach ( var size in ConfiguratorIconSizes )
        {
            await InstallData( context, context.Path( RepositoryPaths.PackagingDirectory, "icons", $"wayscriber-configurator-{size}.png" ), Path.Combine( dataDir, $"icons/hicolor/{size}x{size}/apps/wayscriber-configurator.png" ) );
        }
        await InstallData( context, context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-configurator.svg" ), Path.Combine( dataDir, "icons/hicolor/scalable/apps/wayscriber-configurator.svg" ) );
        await InstallData( context, context.Path( RepositoryPaths.PackagingDirectory, "icons", "wayscriber-configurator-128.png" ), Path.Combine( dataDir, "pixmaps/wayscriber-configurator.png" ) );
        if ( await CommandExists( context, Programs.UpdateDesktopDatabase ) )
        {
            await RunPossiblyRoot( context, NeedsPrivilege( dataDir ), Programs.UpdateDesktopDatabase, [Path.Combine( dataDir, "applications" )] );
        }
        if ( File.Exists( Path.Combine( dataDir, "icons/hicolor/index.theme" ) ) && await CommandExists( context, Programs.GtkUpdateIconCache ) )
        {
            await RunPossiblyRoot( context, NeedsPrivilege( dataDir ), Programs.GtkUpdateIconCache, ["-q", "-f", "-t", Path.Combine( dataDir, "icons/hicolor" )] );
        }
        await context.Output.WriteLineAsync( $"Configurator installation complete.\nRun: {destination} --help" );
        return ExitCodes.Success;
    }

    internal static string EscapeDesktopExecPath( string path ) => path.Replace( "\\", "\\\\\\\\", StringComparison.Ordinal )
        .Replace( "\"", "\\\\\"", StringComparison.Ordinal ).Replace( "`", "\\\\`", StringComparison.Ordinal )
        .Replace( "$", "\\\\$", StringComparison.Ordinal ).Replace( "%", "%%", StringComparison.Ordinal );

    private static async Task InstallSystemdService( ToolContext context, string destination, string installDir, string userDirectory )
    {
        var userService = Path.Combine( userDirectory, RepositoryNames.UserServiceFile );
        var systemService = "/usr/lib/systemd/user/wayscriber.service";
        var target = installDir == "/usr/bin" ? systemService : userService;
        if ( File.Exists( userService ) && target != userService )
        {
            File.Delete( userService );
        }
        var service = Files.Read( context.Path( RepositoryPaths.PackagingDirectory, RepositoryNames.UserServiceFile ) );
        if ( target == userService )
        {
            service = ReplaceExactlyOnce( service, "ExecStart=/usr/bin/wayscriber --daemon", $"ExecStart={destination} --daemon", "service ExecStart" );
            service = ReplaceExactlyOnce( service, "Environment=\"PATH=/usr/local/bin:/usr/bin:/bin\"", $"Environment=\"PATH={installDir}:/usr/local/bin:/usr/bin:/bin\"", "service PATH" );
        }
        using var temporary = new TemporaryDirectory( "wayscriber-service" );
        var source = Path.Combine( temporary.Path, RepositoryNames.UserServiceFile );
        Files.WriteAtomic( source, service );
        await RunPossiblyRoot( context, NeedsPrivilege( Path.GetDirectoryName( target )! ), Programs.Install, [InstallArguments.DataFile, source, target] );
        await context.Run( Programs.SystemControl, ["--user", "daemon-reload"] );
        await context.Run( Programs.SystemControl, ["--user", "enable", RepositoryNames.UserServiceFile] );
        var restart = await context.Run( Programs.SystemControl, ["--user", "restart", RepositoryNames.UserServiceFile], allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        if ( restart.ExitCode != ExitCodes.Success )
        {
            await context.Run( Programs.SystemControl, ["--user", "start", RepositoryNames.UserServiceFile] );
        }
    }

    private static void ConfigureHyprland( string home, string destination, bool includeAutostart )
    {
        var path = Path.Combine( home, ".config/hypr/hyprland.conf" );
        if ( !File.Exists( path ) )
        {
            throw new ToolException( $"Hyprland config not found: {path}" );
        }
        var text = Files.Read( path );
        var additions = new List<string>( );
        if ( includeAutostart && !text.Contains( "wayscriber --daemon", StringComparison.Ordinal ) && !text.Contains( $"{destination} --daemon", StringComparison.Ordinal ) )
        {
            additions.Add( $"exec-once = {destination} --daemon" );
        }
        if ( !text.Contains( "wayscriber --daemon-toggle", StringComparison.Ordinal ) && !text.Contains( "pkill -SIGUSR1 wayscriber", StringComparison.Ordinal ) )
        {
            additions.Add( $"bind = SUPER, D, exec, {destination} --daemon-toggle" );
        }
        if ( additions.Count > 0 )
        {
            Files.WriteAtomic( path, text.TrimEnd( ) + "\n\n# wayscriber\n" + string.Join( '\n', additions ) + "\n" );
        }
    }

    private static bool HasConflictingExecStart( string path, string destination, IEnumerable<string> known )
    {
        if ( !File.Exists( path ) )
        {
            return false;
        }
        var lines = File.ReadLines( path ).Where( line => line.TrimStart( ).StartsWith( "ExecStart=", StringComparison.Ordinal ) ).ToArray( );
        return lines.Length > 0 && !lines.Any( line => line.Contains( destination, StringComparison.Ordinal ) ) && known.Any( binary => lines.Any( line => line.Contains( binary, StringComparison.Ordinal ) ) );
    }

    private static bool SameFile( string left, string right )
    {
        if ( !File.Exists( left ) || !File.Exists( right ) )
        {
            return false;
        }
        return Path.GetFullPath( File.ResolveLinkTarget( left, true )?.FullName ?? left ) == Path.GetFullPath( File.ResolveLinkTarget( right, true )?.FullName ?? right );
    }

    private static async Task<bool> IsPackageOwned( ToolContext context, string path )
    {
        if ( !await CommandExists( context, Programs.Pacman ) )
        {
            return false;
        }
        var result = await context.Run( Programs.Pacman, ["-Qoq", path], capture: true, trace: false, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );
        return result.ExitCode == ExitCodes.Success;
    }

    private static async Task<bool> CommandExists( ToolContext context, string command ) =>
        (await context.Run( Programs.Which, [command], capture: true, trace: false, allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } )).ExitCode == ExitCodes.Success;

    private static string ReplaceExactlyOnce( string text, string oldValue, string newValue, string label )
    {
        if ( text.Split( oldValue ).Length != 2 )
        {
            throw new ToolException( $"Expected exactly one {label}." );
        }
        return text.Replace( oldValue, newValue, StringComparison.Ordinal );
    }

    private static Task<ProcessResult> InstallData( ToolContext context, string source, string destination ) =>
        RunPossiblyRoot( context, NeedsPrivilege( Path.GetDirectoryName( destination )! ), Programs.Install, [InstallArguments.DataFile, source, destination] );

    internal static string NormalizeBinDirectory( string value )
    {
        var path = Path.GetFullPath( Path.TrimEndingDirectorySeparator( value ) );
        return path switch
        {
            "/usr" => "/usr/bin",
            "/usr/local" => "/usr/local/bin",
            _ => path
        };
    }

    private static bool NeedsPrivilege( string path )
    {
        var current = Path.GetFullPath( path );
        while ( !Directory.Exists( current ) && Path.GetDirectoryName( current ) is { } parent && parent != current )
        {
            current = parent;
        }
        try
        {
            using var stream = new FileStream( Path.Combine( current, $".wayscriber-write-{Guid.NewGuid( ):N}" ), FileMode.CreateNew, FileAccess.Write, FileShare.None, 1, FileOptions.DeleteOnClose );
            return false;
        }
        catch ( UnauthorizedAccessException ) { return true; }
    }

    internal static bool ShouldUseSudo( bool requiresPrivilege, uint effectiveUserId ) =>
        requiresPrivilege && effectiveUserId != RootUserId;

    [System.Runtime.InteropServices.DllImport( "libc", EntryPoint = "geteuid" )]
    private static extern uint GetEffectiveUserId( );

    private static Task<ProcessResult> RunPossiblyRoot( ToolContext context, bool requiresPrivilege, string program, IReadOnlyList<string> arguments,
        IReadOnlySet<int>? allowedExitCodes = null ) => ShouldUseSudo( requiresPrivilege, GetEffectiveUserId( ) )
        ? context.Run( Programs.Sudo, [program, .. arguments], allowedExitCodes: allowedExitCodes )
        : context.Run( program, arguments, allowedExitCodes: allowedExitCodes );
}
