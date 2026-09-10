namespace Wayscriber.Tools;

internal static class DevelopmentCommands
{
    private const int WestonStartupAttempts = 100;
    private const int WestonPollDelayMilliseconds = 100;
    private static readonly string[] CsharpFileApps = ["tools/wayscriber.cs", "tools/install.cs", "tools/wayscriber.tests.cs"];
    private static readonly string[] CsharpFormatModes = ["style", "whitespace"];

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new(CommandAreas.Development, CommandNames.Build, "Build release binaries.", SideEffect.ReadOnly, Build),
        new(CommandAreas.Development, CommandNames.Test, "Run workspace tests.", SideEffect.ReadOnly, Test),
        new(CommandAreas.Development, CommandNames.Fetch, "Fetch locked Cargo dependencies.", SideEffect.ReadOnly, Fetch),
        new(CommandAreas.Development, CommandNames.FormatAndLint, "Format and lint the workspace with all features.",
            SideEffect.FixtureMutating, FormatAndLint),
        new(CommandAreas.ContinuousIntegration, CommandNames.LintAndTest, "Run the canonical repository gate.", SideEffect.ReadOnly,
            LintAndTest),
        new(CommandAreas.ContinuousIntegration, CommandNames.GtkWidgets, "Run GTK contracts on a private Weston display.",
            SideEffect.ForegroundSensitive, GtkWidgets),
        new(CommandAreas.ContinuousIntegration, CommandNames.InstallDependencies, "Install CI system packages.", SideEffect.MachineMutating,
            InstallDependencies),
        new(CommandAreas.ContinuousIntegration, CommandNames.PrepareGtk4LayerShell, "Install gtk4-layer-shell and export its CI paths.",
            SideEffect.MachineMutating, PrepareGtkLayerShell),
        new(CommandAreas.ContinuousIntegration, CommandNames.BuildLinkage, "Build Wayscriber and verify native linkage.",
            SideEffect.ReadOnly, BuildLinkage),
        new(CommandAreas.ContinuousIntegration, CommandNames.RequireEnvironment, "Fail when required environment values are empty.",
            SideEffect.ReadOnly, RequireEnvironment),
        new(CommandAreas.Check, CommandNames.NixVersions, "Compare Nix package versions with Cargo.", SideEffect.ReadOnly, NixVersions),
        new(CommandAreas.Check, CommandNames.NixInstantiation, "Instantiate Nix packages and the development shell.", SideEffect.ReadOnly,
            NixInstantiation),
    ];

    private static async Task<int> Build( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "dev build" );
        await context.Output.WriteLineAsync( "Building wayscriber (default features)..." );
        await context.Run( Programs.Cargo, ["build", CommandLineOptions.Release, CommandLineOptions.Binaries] );
        await context.Output.WriteLineAsync( "Build complete." );
        return ExitCodes.Success;
    }

    private static async Task<int> Test( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "dev test" );
        await context.Output.WriteLineAsync( "Running tests (default features)..." );
        await context.Run( Programs.Cargo, ["test", CommandLineOptions.Workspace] );
        await context.Output.WriteLineAsync( "Tests complete." );
        return ExitCodes.Success;
    }

    private static async Task<int> Fetch( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var target = parsed.TakeOption( "--target" );
        parsed.RequireEmpty( "dev fetch [--target TRIPLE]" );
        await context.Output.WriteLineAsync( "Fetching wayscriber dependencies..." );
        await FetchManifest( context, RepositoryPaths.CargoManifest, target );
        if ( File.Exists( context.Path( RepositoryPaths.ConfiguratorCargoManifest ) ) )
        {
            await context.Output.WriteLineAsync( "Fetching configurator dependencies..." );
            await FetchManifest( context, RepositoryPaths.ConfiguratorCargoManifest, target );
        }
        await context.Output.WriteLineAsync( "Dependency fetch complete." );
        return ExitCodes.Success;
    }

    private static async Task<int> FormatAndLint( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "dev format-and-lint" );
        await context.Run( Programs.Cargo, ["fmt"] );
        await context.Run( Programs.Cargo, ["clippy", CommandLineOptions.Workspace, CommandLineOptions.AllFeatures] );
        return ExitCodes.Success;
    }

    private static Task<ProcessResult> FetchManifest( ToolContext context, string manifest, string? target )
    {
        var arguments = new List<string> { "fetch", CommandLineOptions.Locked, "--manifest-path", context.Path( manifest.Split( '/' ) ) };
        if ( target is not null )
        {
            arguments.AddRange( ["--target", target] );
        }
        return context.Run( Programs.Cargo, arguments );
    }

    private static async Task<int> LintAndTest( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "ci lint-and-test" );
        await RunTool( context, CommandAreas.Assets, CommandNames.Check );
        await RunTool( context, CommandAreas.Version, CommandNames.Check );
        await RunTool( context, CommandAreas.Check, CommandNames.NixpkgsRecipe );
        await RunTool( context, CommandAreas.Check, CommandNames.RustSourceCoverage );
        await RunTool( context, CommandAreas.Check, CommandNames.ProcessSites );
        await RunTool( context, CommandAreas.Check, CommandNames.ConfigWriters );
        await RunTool( context, CommandAreas.Check, CommandNames.SharedDependencies );
        await RunTool( context, CommandAreas.Check, CommandNames.LegacyTools );
        await RunCsharpFormattingChecks( context );
        await context.Run( Programs.Dotnet, ["run", "tools/wayscriber.tests.cs", "--no-build", CommandLineOptions.EndOfOptions] );
        await RunCargo( context, ["fmt", "--all", CommandLineOptions.EndOfOptions, "--check"] );
        await RunCargo( context, ["clippy", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.AllTargets, CommandLineOptions.AllFeatures, CommandLineOptions.EndOfOptions, "-D", "warnings"] );
        await RunCargo( context, ["build", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.AllFeatures, CommandLineOptions.Binaries] );
        await RunCargo( context, ["test", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.AllFeatures, CommandLineOptions.EndOfOptions, CommandLineOptions.SingleTestThread] );
        await RunIsolatedRenderTests( context, CommandLineOptions.AllFeatures );
        await RunCargo( context, ["clippy", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.AllTargets, CommandLineOptions.NoDefaultFeatures, CommandLineOptions.EndOfOptions, "-D", "warnings"] );
        await RunCargo( context, ["build", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.NoDefaultFeatures, CommandLineOptions.Binaries] );
        await RunCargo( context, ["test", CommandLineOptions.Locked, CommandLineOptions.Workspace, CommandLineOptions.NoDefaultFeatures, CommandLineOptions.EndOfOptions, CommandLineOptions.SingleTestThread] );
        await RunIsolatedRenderTests( context, CommandLineOptions.NoDefaultFeatures );
        return ExitCodes.Success;
    }

    private static async Task RunCsharpFormattingChecks( ToolContext context )
    {
        foreach ( var fileApp in CsharpFileApps )
        {
            foreach ( var formatMode in CsharpFormatModes )
            {
                await context.Run( Programs.Dotnet,
                    ["format", formatMode, fileApp, CommandLineOptions.NoRestore, CommandLineOptions.VerifyNoChanges] );
            }
        }
    }

    private static async Task RunTool( ToolContext context, string area, string command )
    {
        await context.Output.WriteLineAsync( $"\nRunning: wayscriber {area} {command}" );
        var result = await ToolApplication.RunNestedAsync( context, area, command, [] );
        if ( result != ExitCodes.Success )
        {
            throw new ToolException( $"{area} {command} failed.", result );
        }
    }

    private static async Task RunCargo( ToolContext context, IReadOnlyList<string> arguments )
    {
        await context.Output.WriteLineAsync( $"\nRunning: {ProcessRunner.FormatCommand( Programs.Cargo, arguments )}" );
        await context.Run( Programs.Cargo, arguments, trace: false );
    }

    private static async Task RunIsolatedRenderTests( ToolContext context, string feature )
    {
        string[] tests =
        [
            "ui::context_menu::engine_tests::retained_context_menu_owner_preserves_layout_pixels_and_row_hits",
            "ui::board_picker::tests::retained_board_text_owner_matches_fresh_during_unicode_rename_and_small_layouts",
        ];
        foreach ( var test in tests )
        {
            var result = await context.Run( Programs.Cargo, ["test", CommandLineOptions.Locked, "-p", RepositoryNames.MainPackage, feature, "--lib", test,
                CommandLineOptions.EndOfOptions, "--exact", "--ignored", CommandLineOptions.SingleTestThread], capture: true );
            await context.Output.WriteAsync( result.StandardOutput );
            if ( !result.StandardOutput.Contains( "test result: ok. 1 passed; 0 failed;", StringComparison.Ordinal ) )
            {
                throw new ToolException( $"Expected exactly one passing isolated render test: {test} ({feature})" );
            }
        }
    }

    private static async Task<int> GtkWidgets( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "ci gtk-widgets" );
        using var runtime = new TemporaryDirectory( "wayscriber-gtk-tests" );
        const string display = "wayscriber-widget-tests";
        var environment = new Dictionary<string, string?>
        {
            [EnvironmentVariables.XdgRuntimeDirectory] = runtime.Path,
            [EnvironmentVariables.WaylandDisplay] = display,
            [EnvironmentVariables.Display] = null,
            [EnvironmentVariables.GdkBackend] = "wayland",
            [EnvironmentVariables.GtkAccessibility] = "test",
            [EnvironmentVariables.WayscriberRequireGtkTests] = EnvironmentVariables.Enabled,
        };
        using var westonCancellation = CancellationTokenSource.CreateLinkedTokenSource( context.CancellationToken );
        var weston = context.Processes.RunAsync( new ProcessRequest( Programs.Weston,
            ["--backend=headless-backend.so", "--renderer=pixman", "--no-config", $"--socket={display}", "--idle-time=0", $"--log={Path.Combine( runtime.Path, "weston.log" )}"],
            context.RepositoryRoot, environment, CaptureOutput: true ), westonCancellation.Token );
        try
        {
            var socket = Path.Combine( runtime.Path, display );
            for ( var attempt = 0; attempt < WestonStartupAttempts && !File.Exists( socket ); attempt++ )
            {
                if ( weston.IsCompleted )
                {
                    _ = await weston;
                    throw new ToolException( Files.Read( Path.Combine( runtime.Path, "weston.log" ) ) );
                }
                await Task.Delay( WestonPollDelayMilliseconds, context.CancellationToken );
            }
            if ( !File.Exists( socket ) )
            {
                throw new ToolException( "Weston did not create its Wayland socket." );
            }
            var result = await context.Run( Programs.DbusRunSession, [CommandLineOptions.EndOfOptions, Programs.Cargo, CommandNames.Test, CommandLineOptions.Locked, "-p",
                RepositoryNames.MainPackage, CommandLineOptions.AllFeatures, "--lib",
                "toolbar_gtk::", CommandLineOptions.EndOfOptions, CommandLineOptions.SingleTestThread, "--nocapture"], environment: environment, capture: true );
            await context.Output.WriteAsync( result.StandardOutput );
            await context.Error.WriteAsync( result.StandardError );
            var combinedOutput = result.StandardOutput + result.StandardError;
            foreach ( var marker in new[] { "EXECUTED: GTK focus and slider assertions", "EXECUTED: GTK widget contract assertions" } )
            {
                if ( !combinedOutput.Contains( marker, StringComparison.Ordinal ) )
                {
                    throw new ToolException( $"GTK test output lacks marker: {marker}" );
                }
            }
            return ExitCodes.Success;
        }
        finally
        {
            westonCancellation.Cancel( );
            try
            {
                _ = await weston;
            }
            catch ( OperationCanceledException ) { }
        }
    }

    private static async Task<int> InstallDependencies( ToolContext context, string[] args )
    {
        var profile = new Arguments( args ).SinglePositional( "ci install-dependencies checks|widgets|package|repositories" );
        var common = new[] { "build-essential", "pkg-config", "libwayland-dev", "libxkbcommon-dev", "libcairo2-dev", "libpango1.0-dev",
            "libgtk-3-dev", "libgtk-4-dev", "libadwaita-1-dev", "meson", "ninja-build", "wayland-protocols", "libssl-dev",
            "libxcb-shape0-dev", "libxcb-xfixes0-dev", "libxcb-render0-dev", "libxcb1-dev", "libx11-dev" };
        var repositoryPackages = new[] { "apt-utils", "dpkg-dev", "rpm", "createrepo-c", "rsync", "gnupg" };
        IEnumerable<string>? packages = profile switch
        {
            "checks" => common.Concat( ["clang", "cmake", "libxkbcommon-x11-dev", "libegl1-mesa-dev", "libgles2-mesa-dev", "libdbus-1-dev", "libinput-dev", "libudev-dev", "libpixman-1-dev", "libxcb-randr0-dev"] )
                .Concat( repositoryPackages ),
            "widgets" => common.Concat( ["weston", "dbus-x11", "fonts-dejavu-core", "clang", "cmake", "libxkbcommon-x11-dev", "libegl1-mesa-dev", "libgles2-mesa-dev", "libdbus-1-dev", "libinput-dev", "libudev-dev", "libpixman-1-dev", "libxcb-randr0-dev"] ),
            "package" => common.Concat( ["rpm"] ),
            "repositories" => repositoryPackages,
            _ => null,
        };
        if ( packages is null )
        {
            throw new ToolException( $"Unknown dependency profile: {profile}", ExitCodes.InvalidArguments );
        }

        await context.Run( Programs.Sudo, [Programs.AptGet, "update"] );
        await context.Run( Programs.Sudo, [Programs.AptGet, "install", "-y", .. packages] );
        return ExitCodes.Success;
    }

    private static async Task<int> PrepareGtkLayerShell( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var prefix = parsed.TakeOption( "--prefix" ) ?? (context.Environment( EnvironmentVariables.RunnerTemporary ) is { Length: > 0 } runner
            ? Path.Combine( runner, RepositoryNames.Gtk4LayerShell )
            : context.Path( RepositoryPaths.TargetDirectory, "ci-deps", RepositoryNames.Gtk4LayerShell ));
        var mode = parsed.TakeOption( "--library-mode" ) ?? NativeLibraryModes.Both;
        var githubEnvironment = parsed.TakeOption( "--github-env" ) ?? context.Environment( EnvironmentVariables.GitHubEnvironment );
        parsed.RequireEmpty( "ci prepare-gtk4-layer-shell [--prefix PATH] [--library-mode MODE] [--github-env FILE]" );
        await ToolApplication.RunNestedAsync( context, CommandAreas.Native, CommandNames.InstallGtk4LayerShell, ["--prefix", prefix, "--library-mode", mode] );
        if ( githubEnvironment is not null )
        {
            var pkg = Path.Combine( prefix, "lib/pkgconfig" ) + (context.Environment( EnvironmentVariables.PackageConfigPath ) is { Length: > 0 } oldPkg ? $":{oldPkg}" : string.Empty);
            var library = Path.Combine( prefix, "lib" ) + (context.Environment( EnvironmentVariables.LibraryPath ) is { Length: > 0 } oldLibrary ? $":{oldLibrary}" : string.Empty);
            await File.AppendAllTextAsync( githubEnvironment, $"PKG_CONFIG_PATH={pkg}\nLD_LIBRARY_PATH={library}\n", context.CancellationToken );
        }
        return ExitCodes.Success;
    }

    private static async Task<int> BuildLinkage( ToolContext context, string[] args )
    {
        var mode = new Arguments( args ).SinglePositional( "ci build-linkage dynamic|static" );
        if ( mode is not (NativeLibraryModes.Dynamic or NativeLibraryModes.Static) )
        {
            throw new ToolException( "ci build-linkage dynamic|static", ExitCodes.InvalidArguments );
        }
        IReadOnlyDictionary<string, string?>? environment = mode == NativeLibraryModes.Static
            ? new Dictionary<string, string?> { [EnvironmentVariables.SystemGtk4LayerShellLink] = NativeLibraryModes.Static }
            : null;
        await context.Run( Programs.Cargo, ["build", CommandLineOptions.Locked, "--bin", RepositoryNames.MainPackage], environment: environment );
        var verification = mode == NativeLibraryModes.Static ? CommandNames.VerifyStatic : CommandNames.VerifyDynamic;
        return await ToolApplication.RunNestedAsync( context, CommandAreas.Elf, verification,
            [context.Path( RepositoryPaths.TargetDirectory, RepositoryPaths.DebugDirectory, RepositoryNames.MainPackage )] );
    }

    private static Task<int> RequireEnvironment( ToolContext context, string[] args )
    {
        if ( args.Length == 0 )
        {
            throw new ToolException( "ci require-environment NAME [NAME ...]", ExitCodes.InvalidArguments );
        }
        var missing = args.Where( name => string.IsNullOrWhiteSpace( context.Environment( name ) ) ).ToArray( );
        if ( missing.Length > 0 )
        {
            throw new ToolException( $"Required environment values are missing: {string.Join( ", ", missing )}" );
        }
        return Task.FromResult( ExitCodes.Success );
    }

    private static async Task<int> NixVersions( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check nix-versions" );
        var cargoVersion = VersionCommands.ReadCargoVersion( context.Path( RepositoryPaths.CargoManifest ) );
        foreach ( var attribute in new[] { ".#packages.x86_64-linux.wayscriber.version", ".#packages.x86_64-linux.wayscriber-configurator.version" } )
        {
            var result = await context.Run( Programs.Nix, ["eval", attribute, "--raw"], capture: true );
            if ( result.StandardOutput.Trim( ) != cargoVersion )
            {
                throw new ToolException( $"{attribute}: expected {cargoVersion}, got {result.StandardOutput.Trim( )}." );
            }
        }
        return ExitCodes.Success;
    }

    private static async Task<int> NixInstantiation( ToolContext context, string[] args )
    {
        new Arguments( args ).RequireEmpty( "check nix-instantiation" );
        await context.Run( Programs.Nix, ["build", ".#wayscriber", ".#wayscriber-configurator", ".#devShells.x86_64-linux.default", "--no-link", "--dry-run"] );
        return ExitCodes.Success;
    }
}
