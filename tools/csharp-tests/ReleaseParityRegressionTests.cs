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
    private const string DotnetLogVariable = "WAYSCRIBER_DOTNET_LOG";

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
        var recipe = AssetsCommand.CreateRecipe( FindRepository( ) )["source"]!.AsObject( );
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
        var recipe = AssetsCommand.CreateRecipe( FindRepository( ) )[channel]!.AsObject( );
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
        var recipe = AssetsCommand.CreateRecipe( FindRepository( ) )[channel]!.AsObject( );
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

    [Fact]
    public async Task StandaloneGateUsesSdkResolutionAndRunsEveryCsharpBuild( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = CreateStandaloneGateFixture( dotnetSdkAvailable: true );
        var dotnetLog = Path.Combine( fixture.Path, "dotnet.log" );

        var result = await RunStandaloneGate( fixture.Path, dotnetLog, new HashSet<int> { ExitCodes.Success } );

        Assert.Equal( ExitCodes.Success, result.ExitCode );
        var invocations = File.ReadAllText( dotnetLog );
        Assert.Contains( "build tools/wayscriber.cs --disable-build-servers --verbosity quiet", invocations, StringComparison.Ordinal );
        Assert.Contains( "build tools/install.cs --disable-build-servers --verbosity quiet", invocations, StringComparison.Ordinal );
        Assert.Contains( "build tools/wayscriber.tests.cs --disable-build-servers --verbosity quiet", invocations, StringComparison.Ordinal );
        Assert.Contains( "format style tools/wayscriber.cs --no-restore --verify-no-changes", invocations, StringComparison.Ordinal );
        Assert.Contains( "format whitespace tools/install.cs --no-restore --verify-no-changes", invocations, StringComparison.Ordinal );
        Assert.Contains( "format whitespace tools/wayscriber.tests.cs --no-restore --verify-no-changes", invocations, StringComparison.Ordinal );
        Assert.Contains( "run tools/wayscriber.tests.cs --no-build --verbosity quiet", invocations, StringComparison.Ordinal );
    }

    [Fact]
    public async Task StandaloneGateSkipsOnlyCsharpChecksWhenPinnedSdkIsUnavailable( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = CreateStandaloneGateFixture( dotnetSdkAvailable: false );
        var dotnetLog = Path.Combine( fixture.Path, "dotnet.log" );

        var result = await RunStandaloneGate( fixture.Path, dotnetLog, new HashSet<int> { ExitCodes.Success } );

        Assert.Equal( ExitCodes.Success, result.ExitCode );
        Assert.Contains( "SDK selected by global.json is unavailable", result.StandardOutput, StringComparison.Ordinal );
        Assert.False( File.Exists( dotnetLog ) );
    }

    private static ToolContext CreateContext( IProcessRunner runner, Func<string, string?>? environment = null ) =>
        new( FindRepository( ), TextWriter.Null, TextWriter.Null, runner, CancellationToken.None, environment );

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
    private static TemporaryDirectory CreateStandaloneGateFixture( bool dotnetSdkAvailable )
    {
        var fixture = new TemporaryDirectory( "wayscriber-standalone-gate-test" );
        var tools = Path.Combine( fixture.Path, "tools" );
        var fakeBin = Path.Combine( fixture.Path, "fake-bin" );
        Directory.CreateDirectory( tools );
        Directory.CreateDirectory( fakeBin );
        File.Copy( Path.Combine( FindRepository( ), "tools/lint-and-test.sh" ), Path.Combine( tools, "lint-and-test.sh" ) );
        File.Copy( Path.Combine( FindRepository( ), "global.json" ), Path.Combine( fixture.Path, "global.json" ) );
        File.CreateSymbolicLink( Path.Combine( fakeBin, "bash" ), "/usr/bin/true" );
        WriteExecutable( Path.Combine( fakeBin, "cargo" ), "#!/usr/bin/bash\nprintf 'test result: ok. 1 passed; 0 failed;\\n'\n" );
        WriteExecutable( Path.Combine( fakeBin, "dotnet" ), $$"""
#!/usr/bin/bash
if [[ "$1" == "--version" ]]; then
    {{(dotnetSdkAvailable ? $"printf '%s\\n' '{TestDotnetSdk}'" : "exit 1")}}
else
    printf '%s\n' "$*" >> "${WAYSCRIBER_DOTNET_LOG:?}"
fi
""" );
        foreach ( var check in new[] { "check-nixpkgs-recipe.py", "check-rust-source-coverage.py", "check-process-sites.py",
            "check-config-writers.py", "check-shared-dependencies.py" } )
        {
            WriteExecutable( Path.Combine( tools, check ), "#!/usr/bin/true\n" );
        }

        return fixture;
    }

    [SupportedOSPlatform( "linux" )]
    private static void WriteExecutable( string path, string content )
    {
        File.WriteAllText( path, content );
        File.SetUnixFileMode( path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
    }

    private static Task<ProcessResult> RunStandaloneGate( string repository, string dotnetLog, IReadOnlySet<int> allowedExitCodes )
    {
        var environment = new Dictionary<string, string?>
        {
            [EnvironmentVariables.Path] = Path.Combine( repository, "fake-bin" ) + Path.PathSeparator +
                Environment.GetEnvironmentVariable( EnvironmentVariables.Path ),
            [DotnetLogVariable] = dotnetLog,
        };
        var request = new ProcessRequest( "/usr/bin/bash", [Path.Combine( repository, "tools/lint-and-test.sh" )], repository,
            environment, CaptureOutput: true, Trace: false, AllowedExitCodes: allowedExitCodes );
        return new ProcessRunner( TextWriter.Null, TextWriter.Null ).RunAsync( request, CancellationToken.None );
    }

    private static string FindRepository( )
    {
        var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
        return Path.GetFullPath( Path.Combine( directory, ".." ) );
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
