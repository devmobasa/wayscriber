namespace Wayscriber.Tools;

internal static class DevelopmentCommands
{
    private const int WestonStartupAttempts = 100;
    private const int WestonPollDelayMilliseconds = 100;
    private const string CsharpTestApp = "tools/wayscriber.tests.cs";
    private static readonly string[] CsharpFileApps = ["tools/wayscriber.cs", "tools/install.cs", CsharpTestApp];
    private static readonly string[] CsharpFormatModes = ["style", "whitespace"];
    private static readonly string[] IsolatedRenderTests =
    [
        "ui::context_menu::engine_tests::retained_context_menu_owner_preserves_layout_pixels_and_row_hits",
        "ui::board_picker::tests::retained_board_text_owner_matches_fresh_during_unicode_rename_and_small_layouts",
    ];

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

        foreach ( var step in LintAndTestPlan )
        {
            await RunLintStep( context, step );
        }

        return ExitCodes.Success;
    }

    // `ci lint-and-test`, in order. After building the C# apps, `tools/lint-and-test.sh`
    // runs the same steps, and the C# tests compare its commands with this list.
    internal static IReadOnlyList<LintStep> LintAndTestPlan
    {
        get;
    } = BuildLintAndTestPlan( );

    private static List<LintStep> BuildLintAndTestPlan( )
    {
        List<LintStep> plan =
        [
            new( LintStepKind.RepositoryCheck, [CommandAreas.Assets, CommandNames.Check] ),
            new( LintStepKind.RepositoryCheck, [CommandAreas.Version, CommandNames.Check] ),
            new( LintStepKind.RepositoryCheck, [CommandAreas.Check, CommandNames.NixpkgsRecipe] ),
            new( LintStepKind.RepositoryCheck, [CommandAreas.Check, CommandNames.RustSourceCoverage] ),
            new( LintStepKind.RepositoryCheck, [CommandAreas.Check, CommandNames.LegacyTools] ),
        ];

        foreach ( var fileApp in CsharpFileApps )
        {
            foreach ( var formatMode in CsharpFormatModes )
            {
                plan.Add( new( LintStepKind.Dotnet,
                    ["format", formatMode, fileApp, CommandLineOptions.NoRestore, CommandLineOptions.VerifyNoChanges] ) );
            }
        }

        plan.Add( new( LintStepKind.Dotnet, ["run", CsharpTestApp, "--no-build", "--verbosity", "quiet"] ) );
        plan.Add( new( LintStepKind.Cargo, ["fmt", "--all", CommandLineOptions.EndOfOptions, "--check"] ) );

        foreach ( var features in new[] { CommandLineOptions.AllFeatures, CommandLineOptions.NoDefaultFeatures } )
        {
            string[] workspace = [CommandLineOptions.Locked, CommandLineOptions.Workspace];
            plan.Add( new( LintStepKind.Cargo,
                ["clippy", .. workspace, CommandLineOptions.AllTargets, features, CommandLineOptions.EndOfOptions, "-D", "warnings"] ) );
            plan.Add( new( LintStepKind.Cargo, ["build", .. workspace, features, CommandLineOptions.Binaries] ) );
            plan.Add( new( LintStepKind.Cargo,
                ["test", .. workspace, features, CommandLineOptions.EndOfOptions, CommandLineOptions.SingleTestThread] ) );

            foreach ( var test in IsolatedRenderTests )
            {
                plan.Add( new( LintStepKind.IsolatedRenderTest,
                    ["test", CommandLineOptions.Locked, "-p", RepositoryNames.MainPackage, features, "--lib", test,
                        CommandLineOptions.EndOfOptions, "--exact", "--ignored", CommandLineOptions.SingleTestThread],
                    $"{test} ({features})" ) );
            }
        }

        return plan;
    }

    internal static async Task RunLintStep( ToolContext context, LintStep step )
    {
        switch ( step.Kind )
        {
            case LintStepKind.RepositoryCheck:
                await RunTool( context, step.Arguments[0], step.Arguments[1] );
                break;
            case LintStepKind.Dotnet:
                await context.Run( step.Program!, step.Arguments );
                break;
            case LintStepKind.Cargo:
                await context.Output.WriteLineAsync( $"\nRunning: {ProcessRunner.FormatCommand( step.Program!, step.Arguments )}" );
                await context.Run( step.Program!, step.Arguments, trace: false );
                break;
            case LintStepKind.IsolatedRenderTest:
                await RunIsolatedRenderTest( context, step );
                break;
            default:
                throw new ToolException( $"Unknown lint step: {step.Kind}" );
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

    // Parallel font tests can trigger an upstream Cairo/FreeType race, so these
    // regressions run in their own processes and must each report one passing test.
    private static async Task RunIsolatedRenderTest( ToolContext context, LintStep step )
    {
        var result = await context.Run( step.Program!, step.Arguments, capture: true );
        await context.Output.WriteAsync( result.StandardOutput );

        if ( !result.StandardOutput.Contains( "test result: ok. 1 passed; 0 failed;", StringComparison.Ordinal ) )
        {
            throw new ToolException( $"Expected exactly one passing isolated render test: {step.Description}" );
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
            foreach ( var marker in new[] { "EXECUTED: GTK focus and slider assertions", "EXECUTED: GTK widget contract assertions",
                "EXECUTED: GTK native popup presentation regression", "EXECUTED: GTK native menu presentation assertions" } )
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
            "checks" => common.Concat( ["poppler-utils", "dbus-daemon", "clang", "cmake", "libxkbcommon-x11-dev", "libegl1-mesa-dev", "libgles2-mesa-dev", "libdbus-1-dev", "libinput-dev", "libudev-dev", "libpixman-1-dev", "libxcb-randr0-dev"] )
                .Concat( repositoryPackages ),
            "widgets" => common.Concat( [
                "weston", "dbus-x11", "libgl1-mesa-dri", "fonts-dejavu-core", "clang", "cmake", "libxkbcommon-x11-dev",
                "libegl1-mesa-dev", "libgles2-mesa-dev", "libdbus-1-dev", "libinput-dev", "libudev-dev", "libpixman-1-dev",
                "libxcb-randr0-dev",
            ] ),
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

internal enum LintStepKind
{
    // An in-process repository command: area, then command.
    RepositoryCheck,
    Dotnet,
    Cargo,
    // A Cargo test that must report exactly one passing test.
    IsolatedRenderTest,
}

internal sealed record LintStep( LintStepKind Kind, IReadOnlyList<string> Arguments, string? Description = null )
{
    // The program a process step runs; a repository check runs in process.
    public string? Program => Kind switch
    {
        LintStepKind.Dotnet => Programs.Dotnet,
        LintStepKind.Cargo or LintStepKind.IsolatedRenderTest => Programs.Cargo,
        _ => null,
    };
}
