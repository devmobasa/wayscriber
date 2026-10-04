using System.Runtime.Versioning;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class ReleaseParityRegressionTests
{
    private const string ArchiveRootName = "wayscriber-v1.2.3-linux-x86_64";
    private const string AskpassModeVariable = EnvironmentVariables.WayscriberSshAskpass;
    private const string PassphraseVariable = EnvironmentVariables.AurSshPassphrase;
    private const string SyntheticPassphrase = "synthetic-passphrase";
    private const string RepositoryDeployKey = "repository-deploy-key";
    private const string TestDotnetSdk = "11.0.100-rc.1.26425.128";
    private const string CommandLogVariable = "WAYSCRIBER_COMMAND_LOG";

    [Fact]
    public async Task AurAskpassEnvironmentIsScopedToSshAdd( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-aur-askpass-test" );
        var githubEnvironment = Path.Combine( fixture.Path, "github-environment" );
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.Home] = fixture.Path,
            [EnvironmentVariables.AurSshPrivateKey] = "private-key",
            [EnvironmentVariables.AurSshKnownHosts] = "aur.archlinux.org ssh-ed25519 host-key\n",
            [PassphraseVariable] = SyntheticPassphrase,
            [EnvironmentVariables.GitHubEnvironment] = githubEnvironment,
        };
        var runner = new AskpassAurRunner( );
        var context = CreateContext( runner, name => environment.GetValueOrDefault( name ) );
        var command = ReleaseAurCommands.Commands.Single( item => item.Area == "aur" && item.Name == "prepare-ssh" );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, [] ) );

        var sshAdd = Assert.Single( runner.Requests, request => request.FileName == "setsid" );
        Assert.Equal( "1", sshAdd.Environment![AskpassModeVariable] );
        Assert.Equal( SyntheticPassphrase, sshAdd.Environment[PassphraseVariable] );

        var git = Assert.Single( runner.Requests, request => request.FileName == "git" );
        Assert.False( git.Environment!.ContainsKey( AskpassModeVariable ) );
        Assert.False( git.Environment.ContainsKey( PassphraseVariable ) );
        Assert.Equal( AskpassAurRunner.AgentSocket, git.Environment[EnvironmentVariables.SshAuthSocket] );
        Assert.Equal( AskpassAurRunner.AgentProcessId, git.Environment[EnvironmentVariables.SshAgentProcessId] );

        var persisted = File.ReadAllText( githubEnvironment );
        Assert.Contains( $"SSH_AUTH_SOCK={AskpassAurRunner.AgentSocket}\n", persisted, StringComparison.Ordinal );
        Assert.Contains( $"SSH_AGENT_PID={AskpassAurRunner.AgentProcessId}\n", persisted, StringComparison.Ordinal );
        Assert.Contains( "GIT_SSH_COMMAND=", persisted, StringComparison.Ordinal );
        Assert.DoesNotContain( AskpassModeVariable, persisted, StringComparison.Ordinal );
        Assert.DoesNotContain( PassphraseVariable, persisted, StringComparison.Ordinal );
        Assert.DoesNotContain( SyntheticPassphrase, persisted, StringComparison.Ordinal );
    }

    [Fact]
    public async Task ArchInstallerCheckRejectsUnexpectedTopLevelFiles( )
    {
        using var fixture = CreateArchiveFixture( );
        File.WriteAllText( Path.Combine( fixture.Path, "unexpected.txt" ), "unexpected\n" );
        var archive = await CreateArchive( fixture.Path );

        var error = await RunArchInstallerCheckExpectingFailure( fixture.Path, archive );

        Assert.Contains( "unexpected", error.Message, StringComparison.OrdinalIgnoreCase );
    }

    [Fact]
    public async Task ArchInstallerCheckRejectsASymbolicLinkArchiveRoot( )
    {
        using var fixture = CreateArchiveFixture( );
        using var externalPayload = CreateArchiveFixture( );
        var archiveRoot = Path.Combine( fixture.Path, ArchiveRootName );
        Directory.Delete( archiveRoot, recursive: true );
        Directory.CreateSymbolicLink( archiveRoot, Path.Combine( externalPayload.Path, ArchiveRootName ) );
        var archive = await CreateArchive( fixture.Path );

        var error = await RunArchInstallerCheckExpectingFailure( fixture.Path, archive );

        Assert.Contains( "symbolic link or special file", error.Message, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( "hard-link" )]
    [InlineData( "fifo" )]
    public async Task ArchInstallerCheckRejectsUnsupportedFileTypes( string kind )
    {
        using var fixture = CreateArchiveFixture( );
        var usr = Path.Combine( fixture.Path, ArchiveRootName, "usr" );
        if ( kind == "hard-link" )
        {
            var source = Path.Combine( usr, "bin/wayscriber" );
            var link = Path.Combine( usr, "bin/wayscriber-copy" );
            await RunProcess( fixture.Path, "ln", [source, link] );
        }
        else
        {
            var specialDirectory = Path.Combine( usr, "share/wayscriber" );
            Directory.CreateDirectory( specialDirectory );
            await RunProcess( fixture.Path, "mkfifo", [Path.Combine( specialDirectory, "release-pipe" )] );
        }
        var archive = await CreateArchive( fixture.Path );

        var error = await RunArchInstallerCheckExpectingFailure( fixture.Path, archive );

        var expected = kind == "hard-link" ? "hard-linked file" : "symbolic link or special file";
        Assert.Contains( expected, error.Message, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( "ExecStart=/usr/bin/wayscriber --active\n" )]
    [InlineData( "ExecStartPost=/usr/bin/env wayscriber --active\n" )]
    [InlineData( "ExecStartPost=/usr/bin/env \\\n    wayscriber --active\n" )]
    [InlineData( "ExecStartPost=/usr/bin/env \\\n# ignored by systemd\n; ignored by systemd\n    wayscriber --active\n" )]
    public async Task ArchInstallerCheckRejectsAdditionalWayscriberServiceCommands( string command )
    {
        using var fixture = CreateArchiveFixture( );
        var service = Path.Combine( fixture.Path, ArchiveRootName, "usr/lib/systemd/user/wayscriber.service" );
        File.AppendAllText( service, command );
        var archive = await CreateArchive( fixture.Path );

        var error = await RunArchInstallerCheckExpectingFailure( fixture.Path, archive );

        Assert.Contains( "service is incompatible", error.Message, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( RepositoryDeployKey )]
    [InlineData( RepositoryDeployKey + "\n" )]
    public async Task RepositoryDeployKeyAlwaysEndsWithOnePreservedNewline( string configuredKey )
    {
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.DeployHost] = "packages.example.invalid",
            [EnvironmentVariables.DeployPath] = "/srv/wayscriber",
            [EnvironmentVariables.DeployUser] = "deploy",
            [EnvironmentVariables.PackageRepositorySshKey] = configuredKey,
            [EnvironmentVariables.PackageRepositorySshKnownHosts] = "packages.example.invalid ssh-ed25519 host-key\n",
        };
        var runner = new RepositoryDeployRunner( );
        var context = CreateContext( runner, name => environment.GetValueOrDefault( name ) );
        var command = ReleaseAurCommands.Commands.Single( item =>
            item.Area == CommandAreas.Release && item.Name == CommandNames.DeployPackageRepositories );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, [] ) );

        Assert.Equal( RepositoryDeployKey + "\n", runner.DeployKey );
    }

    [Fact]
    public void ManagedDesktopAssetBlocksReplaceAndRejectStaleEntries( )
    {
        var recipe = AssetsCommand.CreateRecipe( TestRepository.Root )["source"]!.AsObject( );
        var marker = recipe["marker"]!.GetValue<string>( );
        var end = recipe["end_marker"]!.GetValue<string>( );
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var expectedLines = recipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ).ToArray( );
        const string staleLine = "    install -Dm644 packaging/icons/wayscriber-obsolete.png \"$pkgdir/usr/share/icons/hicolor/obsolete/wayscriber.png\"";
        var input = anchor + "\n\n" + marker + "\n" + string.Join( '\n', expectedLines ) + "\n" + staleLine + "\n" + end;

        Assert.False( ReleaseAurCommands.HasExactDesktopAssetBlock( input, recipe ) );

        var result = ReleaseAurCommands.ApplyDesktopAssets( input, recipe );

        Assert.True( ReleaseAurCommands.HasExactDesktopAssetBlock( result, recipe ) );
        Assert.DoesNotContain( staleLine, result, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( PackageChannels.Source )]
    [InlineData( PackageChannels.Binary )]
    [InlineData( PackageChannels.Configurator )]
    public void CompleteMarkerlessDesktopAssetsAreMigratedAndStaleEntriesAreRemoved( string channel )
    {
        var recipe = AssetsCommand.CreateRecipe( TestRepository.Root )[channel]!.AsObject( );
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var expectedLines = recipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ).ToArray( );
        var staleName = channel == PackageChannels.Configurator ? "retired-legacy-icon" : "retired-icon";
        var staleLine = $"    install -Dm644 packaging/icons/{staleName}.png \"$pkgdir/usr/share/icons/hicolor/obsolete/{staleName}.png\"";
        var input = anchor + "\n\n" + string.Join( '\n', expectedLines ) + "\n" + staleLine;

        Assert.False( ReleaseAurCommands.HasExactDesktopAssetBlock( input, recipe ) );

        var result = ReleaseAurCommands.ApplyDesktopAssets( input, recipe );

        Assert.True( ReleaseAurCommands.HasExactDesktopAssetBlock( result, recipe ) );
        Assert.DoesNotContain( staleLine, result, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( PackageChannels.Source )]
    [InlineData( PackageChannels.Binary )]
    [InlineData( PackageChannels.Configurator )]
    public void MarkerlessPreviousManifestSubsetsAreMigrated( string channel )
    {
        var recipe = AssetsCommand.CreateRecipe( TestRepository.Root )[channel]!.AsObject( );
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var previousManifestLines = recipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ).SkipLast( 1 );
        var input = anchor + "\n\n" + string.Join( '\n', previousManifestLines );

        var result = ReleaseAurCommands.ApplyDesktopAssets( input, recipe );

        Assert.True( ReleaseAurCommands.HasExactDesktopAssetBlock( result, recipe ) );
    }

    [Fact]
    public void InstallDirectoryNormalizationPreservesFilesystemRoot( )
    {
        Assert.Equal( Path.GetPathRoot( Environment.CurrentDirectory ), NativeDesktopCommands.NormalizeBinDirectory( "/" ) );
    }

    // The local gate builds the C# apps, then runs `ci lint-and-test`'s steps in the same order.
    [Fact]
    public async Task LocalGateBuildsTheCsharpAppsThenRunsTheCiPlan( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = CreateLocalGateFixture( dotnetSdkAvailable: true );
        var commandLog = Path.Combine( fixture.Path, "commands.log" );

        var result = await RunLocalGate( fixture.Path, commandLog, new HashSet<int> { ExitCodes.Success } );

        Assert.Equal( ExitCodes.Success, result.ExitCode );
        var commands = File.ReadAllLines( commandLog );
        var builds = new[] { "tools/wayscriber.cs", "tools/install.cs", "tools/wayscriber.tests.cs" }
            .Select( app => $"{Programs.Dotnet} build {app} --disable-build-servers --verbosity quiet" );
        // The gate runs a repository check through the built tool; C# runs it in process.
        var plan = DevelopmentCommands.LintAndTestPlan.Select( step => step.Program is { } program
            ? $"{program} {string.Join( ' ', step.Arguments )}"
            : $"{Programs.Dotnet} run tools/wayscriber.cs --no-build -- {string.Join( ' ', step.Arguments )}" );
        Assert.Equal( [.. builds, .. plan], commands );
    }

    [Fact]
    public async Task LocalGateRequiresEachIsolatedRenderTestToPassExactlyOnce( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = CreateLocalGateFixture( dotnetSdkAvailable: true, cargoOutput: "test result: ok. 0 passed; 0 failed;" );
        var commandLog = Path.Combine( fixture.Path, "commands.log" );

        var result = await RunLocalGate( fixture.Path, commandLog, new HashSet<int> { ExitCodes.Failure } );

        Assert.Equal( ExitCodes.Failure, result.ExitCode );
        Assert.Contains( "Expected exactly one passing isolated render test", result.StandardError, StringComparison.Ordinal );
    }

    // Each kind of `ci lint-and-test` step runs what it names: repository checks in
    // process, the others as dotnet or cargo with the step's arguments.
    [Fact]
    public async Task LintStepsRunTheProgramTheirKindNames( )
    {
        var runner = new LintStepRunner( "test result: ok. 1 passed; 0 failed;\n" );
        using var output = new StringWriter( );
        var context = new ToolContext( TestRepository.Root, output, TextWriter.Null, runner, CancellationToken.None );

        await DevelopmentCommands.RunLintStep( context, new( LintStepKind.RepositoryCheck, [CommandAreas.Version, CommandNames.Check] ) );
        await DevelopmentCommands.RunLintStep( context, new( LintStepKind.Dotnet, ["format", "style", "tools/wayscriber.cs"] ) );
        await DevelopmentCommands.RunLintStep( context, new( LintStepKind.Cargo, ["fmt", "--all"] ) );
        await DevelopmentCommands.RunLintStep( context, new( LintStepKind.IsolatedRenderTest, ["test", "--lib", "probe"], "probe" ) );

        Assert.Equal(
            [$"{Programs.Dotnet} format style tools/wayscriber.cs", $"{Programs.Cargo} fmt --all", $"{Programs.Cargo} test --lib probe"],
            runner.Requests.Select( request => $"{request.FileName} {string.Join( ' ', request.Arguments )}" ) );
        Assert.Contains( "Running: wayscriber version check", output.ToString( ), StringComparison.Ordinal );
        Assert.Contains( "Version consistency OK:", output.ToString( ), StringComparison.Ordinal );
    }

    [Fact]
    public async Task AnIsolatedRenderTestMustReportExactlyOnePassingTest( )
    {
        var runner = new LintStepRunner( "test result: ok. 0 passed; 0 failed;\n" );
        var context = new ToolContext( TestRepository.Root, TextWriter.Null, TextWriter.Null, runner, CancellationToken.None );

        var step = new LintStep( LintStepKind.IsolatedRenderTest, ["test", "--lib", "probe"], "probe (--all-features)" );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) => DevelopmentCommands.RunLintStep( context, step ) );

        Assert.Contains( "Expected exactly one passing isolated render test: probe (--all-features)", error.Message,
            StringComparison.Ordinal );
    }

    [Fact]
    public async Task LocalGateFailsBeforeAnyCheckWhenPinnedSdkIsUnavailable( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = CreateLocalGateFixture( dotnetSdkAvailable: false );
        var commandLog = Path.Combine( fixture.Path, "commands.log" );

        var result = await RunLocalGate( fixture.Path, commandLog, new HashSet<int> { ExitCodes.Failure } );

        Assert.Equal( ExitCodes.Failure, result.ExitCode );
        Assert.Contains( "the complete gate needs the .NET SDK selected by global.json", result.StandardError, StringComparison.Ordinal );
        Assert.DoesNotContain( "Running:", result.StandardOutput, StringComparison.Ordinal );
        Assert.False( File.Exists( commandLog ) );
    }

    private static ToolContext CreateContext( IProcessRunner runner, Func<string, string?>? environment = null ) =>
        new( TestRepository.Root, TextWriter.Null, TextWriter.Null, runner, CancellationToken.None, environment );

    private static TemporaryDirectory CreateArchiveFixture( )
    {
        var fixture = new TemporaryDirectory( "wayscriber-archive-validation-test" );
        var usr = Path.Combine( fixture.Path, ArchiveRootName, "usr" );
        Directory.CreateDirectory( Path.Combine( usr, "bin" ) );
        Directory.CreateDirectory( Path.Combine( usr, "lib/systemd/user" ) );
        File.WriteAllText( Path.Combine( usr, "bin/wayscriber" ), "binary\n" );
        File.WriteAllText( Path.Combine( usr, "lib/systemd/user/wayscriber.service" ), "ExecStart=/usr/bin/wayscriber --daemon\n" );
        File.WriteAllText( Path.Combine( fixture.Path, "installer.sh" ), """
# ARCH_INSTALL_MANIFEST_BEGIN
release_manifest() {
    printf '%s\n' \
        '0644 bin/wayscriber' \
        '0644 lib/systemd/user/wayscriber.service'
}
# ARCH_INSTALL_MANIFEST_END
""" );
        return fixture;
    }

    private static async Task<string> CreateArchive( string directory )
    {
        var archive = Path.Combine( directory, "wayscriber.tar.gz" );
        var entries = Directory.EnumerateFileSystemEntries( directory )
            .Select( Path.GetFileName )
            .Where( name => name is not "installer.sh" and not "wayscriber.tar.gz" )
            .Select( name => name! )
            .ToArray( );
        await RunProcess( directory, "tar", ["-czf", archive, "--", .. entries] );
        return archive;
    }

    private static async Task<ToolException> RunArchInstallerCheckExpectingFailure( string directory, string archive )
    {
        var context = CreateContext( new ProcessRunner( TextWriter.Null, TextWriter.Null ) );
        var command = PackagingCommands.Commands.Single( item => item.Area == "package" && item.Name == "check-arch-installer" );
        return await Assert.ThrowsAsync<ToolException>( ( ) => command.Handler(
            context,
            ["--installer", Path.Combine( directory, "installer.sh" ), "--archive", archive] ) );
    }

    private static async Task RunProcess( string directory, string fileName, IReadOnlyList<string> arguments )
    {
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );
        await runner.RunAsync( new ProcessRequest( fileName, arguments, directory, CaptureOutput: true, Trace: false ), CancellationToken.None );
    }

    [SupportedOSPlatform( "linux" )]
    private static TemporaryDirectory CreateLocalGateFixture( bool dotnetSdkAvailable,
        string cargoOutput = "test result: ok. 1 passed; 0 failed;" )
    {
        var fixture = new TemporaryDirectory( "wayscriber-local-gate-test" );
        var tools = Path.Combine( fixture.Path, "tools" );
        var fakeBin = Path.Combine( fixture.Path, "fake-bin" );
        Directory.CreateDirectory( tools );
        Directory.CreateDirectory( fakeBin );
        File.Copy( Path.Combine( TestRepository.Root, "tools/lint-and-test.sh" ), Path.Combine( tools, "lint-and-test.sh" ) );
        File.Copy( Path.Combine( TestRepository.Root, "global.json" ), Path.Combine( fixture.Path, "global.json" ) );
        // Only `tools/lint-and-test.sh` is copied, so any other script the gate runs fails it.
        WriteExecutable( Path.Combine( fakeBin, Programs.Cargo ), $$"""
#!/usr/bin/bash
{{LogInvocation( Programs.Cargo )}}
printf '%s\n' '{{cargoOutput}}'
""" );
        WriteExecutable( Path.Combine( fakeBin, Programs.Dotnet ), $$"""
#!/usr/bin/bash
if [[ "$1" == "--version" ]]; then
    {{(dotnetSdkAvailable ? $"printf '%s\\n' '{TestDotnetSdk}'" : "exit 1")}}
else
    {{LogInvocation( Programs.Dotnet )}}
fi
""" );

        return fixture;
    }

    // Logs one line per invocation: the program, then each argument as `printf %q` quotes
    // it, so an argument the gate splits or joins differently changes the line.
    private static string LogInvocation( string program ) =>
        $"printf '%s%s\\n' '{program}' \"$(printf ' %q' \"$@\")\" >> \"${{{CommandLogVariable}:?}}\"";

    [SupportedOSPlatform( "linux" )]
    private static void WriteExecutable( string path, string content )
    {
        File.WriteAllText( path, content );
        File.SetUnixFileMode( path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
    }

    private static Task<ProcessResult> RunLocalGate( string repository, string commandLog, IReadOnlySet<int> allowedExitCodes )
    {
        var environment = new Dictionary<string, string?>
        {
            [EnvironmentVariables.Path] = Path.Combine( repository, "fake-bin" ) + Path.PathSeparator +
                Environment.GetEnvironmentVariable( EnvironmentVariables.Path ),
            [CommandLogVariable] = commandLog,
        };
        var request = new ProcessRequest( "/usr/bin/bash", [Path.Combine( repository, "tools/lint-and-test.sh" )], repository,
            environment, CaptureOutput: true, Trace: false, AllowedExitCodes: allowedExitCodes );
        return new ProcessRunner( TextWriter.Null, TextWriter.Null ).RunAsync( request, CancellationToken.None );
    }

    // Records each launch and answers it with a fixed standard output.
    private sealed class LintStepRunner( string output ) : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );

            return Task.FromResult( new ProcessResult( ExitCodes.Success, output, string.Empty ) );
        }
    }

    private sealed class AskpassAurRunner : IProcessRunner
    {
        public const string AgentProcessId = "24680";
        public const string AgentSocket = "/tmp/wayscriber-agent.sock";

        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName == "ssh-agent" )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success,
                    $"SSH_AUTH_SOCK={AgentSocket}; export SSH_AUTH_SOCK;\nSSH_AGENT_PID={AgentProcessId}; export SSH_AGENT_PID;\n", string.Empty ) );
            }
            if ( request.FileName == "setsid" || request.FileName == "git" )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
            }

            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class RepositoryDeployRunner : IProcessRunner
    {
        public string? DeployKey { get; private set; }

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            if ( request.FileName == "ssh" )
            {
                var keyArgument = Array.IndexOf( request.Arguments.ToArray( ), "-i" );
                DeployKey = File.ReadAllText( request.Arguments[keyArgument + 1] );
            }
            else if ( request.FileName != "rsync" )
            {
                throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
            }

            return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
        }
    }
}
