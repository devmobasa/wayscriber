using System.Text.Json.Nodes;
using System.Text.RegularExpressions;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class ReleaseRegressionTests
{
    private const string SourceChecksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";

    [Fact]
    public void EmptyAndWhitespaceEnvironmentValuesAreUnset( )
    {
        var values = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            ["EMPTY"] = string.Empty,
            ["WHITESPACE"] = " \t ",
            ["VALUE"] = "configured",
        };
        var context = CreateContext( new RejectingRunner( ), name => values.GetValueOrDefault( name ) );

        Assert.Null( context.Environment( "EMPTY" ) );
        Assert.Null( context.Environment( "WHITESPACE" ) );
        Assert.Equal( "configured", context.Environment( "VALUE" ) );
        Assert.Null( context.Environment( "MISSING" ) );
    }

    [Fact]
    public void InstallerManifestRejectsUnsupportedLinesAmongValidEntries( )
    {
        const string source = """
# ARCH_INSTALL_MANIFEST_BEGIN
release_manifest() {
    printf '%s\n' \
        '0755 bin/wayscriber' \
        "0644 share/wayscriber/unsupported"
}
# ARCH_INSTALL_MANIFEST_END
""";

        var error = Assert.Throws<ToolException>( ( ) => PackagingCommands.ParseInstallerManifest( source ) );

        Assert.Contains( "unsupported installer manifest syntax", error.Message, StringComparison.Ordinal );
    }

    [Theory]
    [InlineData( false )]
    [InlineData( true )]
    public void IncompleteDesktopAssetBlockIsNormalizedWithoutRemovingPackagePayload( bool omitLastAsset )
    {
        var recipe = AssetsCommand.CreateRecipe( FindRepository( ) )["configurator"]!.AsObject( );
        var marker = recipe["marker"]!.GetValue<string>( );
        var endMarker = recipe["end_marker"]!.GetValue<string>( );
        var anchor = recipe["anchor"]!.GetValue<string>( );
        var assetLines = recipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) ).ToArray( );
        var existingAssetLines = omitLastAsset ? assetLines[..^1] : assetLines;
        var payload = "    install -Dm644 README.md \"$pkgdir/usr/share/doc/wayscriber-configurator/README.md\"";
        var input = anchor + "\n\n" + marker + "\n" + string.Join( '\n', existingAssetLines ) + "\n" + payload + "\n}";

        var result = ReleaseAurCommands.ApplyDesktopAssets( input, recipe );

        Assert.Equal( 1, result.Split( '\n' ).Count( line => line == marker ) );
        Assert.Equal( 1, result.Split( '\n' ).Count( line => line == endMarker ) );
        Assert.All( assetLines, line => Assert.Contains( line, result, StringComparison.Ordinal ) );
        Assert.Contains( payload, result, StringComparison.Ordinal );
    }

    [Fact]
    public void IncompleteWayscriberAssetBlockPreservesTheConfiguratorBlock( )
    {
        var recipes = AssetsCommand.CreateRecipe( FindRepository( ) );
        var sourceRecipe = recipes[PackageChannels.Source]!.AsObject( );
        var configuratorRecipe = recipes[PackageChannels.Configurator]!.AsObject( );
        var sourceAnchor = sourceRecipe["anchor"]!.GetValue<string>( );
        var sourceMarker = sourceRecipe["marker"]!.GetValue<string>( );
        var existingSourceAsset = sourceRecipe["lines"]!.AsArray( )[0]!.GetValue<string>( );
        var configuratorAnchor = configuratorRecipe["anchor"]!.GetValue<string>( );
        var configuratorMarker = configuratorRecipe["marker"]!.GetValue<string>( );
        var configuratorEndMarker = configuratorRecipe["end_marker"]!.GetValue<string>( );
        var configuratorAssets = configuratorRecipe["lines"]!.AsArray( ).Select( item => item!.GetValue<string>( ) );
        var configuratorBlock = configuratorMarker + "\n" + string.Join( '\n', configuratorAssets ) + "\n" + configuratorEndMarker;
        var input = sourceAnchor + "\n\n" + sourceMarker + "\n" + existingSourceAsset + "\n\n" +
            configuratorAnchor + "\n\n" + configuratorBlock;

        var result = ReleaseAurCommands.ApplyDesktopAssets( input, sourceRecipe );

        Assert.True( ReleaseAurCommands.HasExactDesktopAssetBlock( result, sourceRecipe ) );
        Assert.True( ReleaseAurCommands.HasExactDesktopAssetBlock( result, configuratorRecipe ) );
        Assert.Contains( configuratorBlock, result, StringComparison.Ordinal );
    }

    [Fact]
    public async Task ConfiguratorUpdateUsesTheGccLibsDependencyAnchor( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-configurator-aur-test" );
        var configurator = Path.Combine( fixture.Path, "configurator" );
        CreateConfiguratorCheckout( configurator );
        var manifest = WriteManifest( fixture.Path );
        var context = CreateContext( new CheckoutOnlyRunner( ) );
        var command = AurUpdateCommand( );

        var result = await command.Handler( context,
            ["--manifest", manifest, "--source-dir", Path.Combine( fixture.Path, "missing-source" ), "--bin-dir", Path.Combine( fixture.Path, "missing-bin" ),
                "--config-dir", configurator, "--source-sha256", SourceChecksum] );

        Assert.Equal( 0, result );
        var packageBuild = File.ReadAllText( Path.Combine( configurator, RepositoryNames.PackageBuildFile ) );
        var sourceInfo = File.ReadAllText( Path.Combine( configurator, RepositoryNames.SourceInfoFile ) );
        Assert.Contains( "    'gtk4'", packageBuild, StringComparison.Ordinal );
        Assert.Contains( "\tdepends = gtk4", sourceInfo, StringComparison.Ordinal );
        Assert.True( packageBuild.IndexOf( "    'gtk4'", StringComparison.Ordinal ) < packageBuild.IndexOf( "    'gcc-libs'", StringComparison.Ordinal ) );
        Assert.True( sourceInfo.IndexOf( "\tdepends = gtk4", StringComparison.Ordinal ) < sourceInfo.IndexOf( "\tdepends = gcc-libs", StringComparison.Ordinal ) );
        Assert.DoesNotContain( "wl-clipboard", packageBuild, StringComparison.Ordinal );
        Assert.DoesNotContain( "wl-clipboard", sourceInfo, StringComparison.Ordinal );
    }

    [Fact]
    public async Task AurPublicationOmitsAnUntrackedObsoleteHookFromTheCommitPathspec( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-aur-publish-test" );
        var source = Path.Combine( fixture.Path, "source" );
        CreateSourceCheckout( source );
        var manifest = WriteManifest( fixture.Path );
        var runner = new PublishingAurRunner( );
        var context = CreateContext( runner );
        var command = AurUpdateCommand( );

        var result = await command.Handler( context,
            ["--manifest", manifest, "--source-dir", source, "--bin-dir", Path.Combine( fixture.Path, "missing-bin" ), "--no-configurator",
                "--source-sha256", SourceChecksum, "--push"] );

        Assert.Equal( 0, result );
        var commit = Assert.Single( runner.Requests, request => request.FileName == "git" && request.Arguments.Contains( "commit" ) );
        Assert.DoesNotContain( "wayscriber.install", commit.Arguments );
        Assert.DoesNotContain( runner.Requests, request => request.FileName == "git" && request.Arguments.Contains( "rm" ) );
    }

    [Fact]
    public async Task AurSshPreparationPreservesUserSshFiles( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-aur-ssh-test" );
        var sshDirectory = Path.Combine( fixture.Path, ".ssh" );
        Directory.CreateDirectory( sshDirectory );
        var knownHostsPath = Path.Combine( sshDirectory, "known_hosts" );
        var configPath = Path.Combine( sshDirectory, "config" );
        File.WriteAllText( knownHostsPath, "existing.example ssh-ed25519 existing\n" );
        File.WriteAllText( configPath, "Host existing.example\n  User existing\n" );
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.Home] = fixture.Path,
            [EnvironmentVariables.AurSshPrivateKey] = "private-key",
            [EnvironmentVariables.AurSshKnownHosts] = string.Empty,
        };
        var runner = new AurSshRunner( );
        var context = CreateContext( runner, name => environment.GetValueOrDefault( name ) );
        var command = ReleaseAurCommands.Commands.Single( item => item.Area == "aur" && item.Name == "prepare-ssh" );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, [] ) );

        Assert.Equal( "existing.example ssh-ed25519 existing\n", File.ReadAllText( knownHostsPath ) );
        Assert.Equal( "Host existing.example\n  User existing\n", File.ReadAllText( configPath ) );
        var dedicatedKnownHosts = Path.Combine( sshDirectory, "known_hosts.wayscriber-aur" );
        Assert.Equal( AurSshRunner.ScannedHostKey, File.ReadAllText( dedicatedKnownHosts ) );
        var request = Assert.Single( runner.Requests, item => item.FileName == "git" );
        Assert.Contains( $"UserKnownHostsFile={dedicatedKnownHosts}", request.Environment![EnvironmentVariables.GitSshCommand], StringComparison.Ordinal );
    }

    [Fact]
    public async Task UnstampedPrivateGtkPrefixIsNeverReused( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-gtk-prefix-test" );
        var prefix = Path.Combine( fixture.Path, "private-prefix" );
        var libraryDirectory = Path.Combine( prefix, "lib" );
        Directory.CreateDirectory( libraryDirectory );
        File.WriteAllText( Path.Combine( libraryDirectory, "libgtk4-layer-shell.so" ), string.Empty );
        var runner = new RejectingRunner( );
        var context = CreateContext( runner );

        var result = await NativeDesktopCommands.GtkArtifactsExist(
            context,
            prefix,
            "/usr",
            "shared",
            Path.Combine( prefix, "share/wayscriber/build-deps/gtk4-layer-shell.pin" ),
            "expected-pin",
            new Dictionary<string, string?>( ) );

        Assert.False( result );
        Assert.Empty( runner.Requests );
    }

    [Fact]
    public async Task UnstampedNestedSystemGtkPrefixIsNeverReused( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-nested-gtk-prefix-test" );
        var systemPrefix = Path.Combine( fixture.Path, "system" );
        var prefix = Path.Combine( systemPrefix, "local" );
        var libraryDirectory = Path.Combine( prefix, "lib" );
        Directory.CreateDirectory( libraryDirectory );
        File.WriteAllText( Path.Combine( libraryDirectory, "libgtk4-layer-shell.so" ), string.Empty );
        var runner = new RejectingRunner( );
        var context = CreateContext( runner );

        var result = await NativeDesktopCommands.GtkArtifactsExist(
            context,
            prefix,
            systemPrefix,
            "shared",
            Path.Combine( prefix, "share/wayscriber/build-deps/gtk4-layer-shell.pin" ),
            "expected-pin",
            new Dictionary<string, string?>( ) );

        Assert.False( result );
        Assert.Empty( runner.Requests );
    }

    [Fact]
    public void NativeInstallUsesSudoOnlyForUnprivilegedCallers( )
    {
        const uint rootUserId = 0;
        const uint unprivilegedUserId = 1_000;

        Assert.False( NativeDesktopCommands.ShouldUseSudo( requiresPrivilege: false, unprivilegedUserId ) );
        Assert.False( NativeDesktopCommands.ShouldUseSudo( requiresPrivilege: true, rootUserId ) );
        Assert.True( NativeDesktopCommands.ShouldUseSudo( requiresPrivilege: true, unprivilegedUserId ) );
    }

    [Fact]
    public async Task FailedBuildPreservesAnExistingInstallation( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-install-build-failure-test" );
        var existingBinary = Path.Combine( fixture.Path, ".local/bin", RepositoryNames.MainPackage );
        Directory.CreateDirectory( Path.GetDirectoryName( existingBinary )! );
        File.WriteAllText( existingBinary, "known-good-installation" );
        var destinationDirectory = Path.Combine( fixture.Path, "replacement", "bin" );
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.Home] = fixture.Path,
            [EnvironmentVariables.XdgConfigHome] = Path.Combine( fixture.Path, ".config" ),
        };
        var runner = new FailingBuildRunner( );
        var context = CreateContext( runner, name => environment.GetValueOrDefault( name ) );
        var command = NativeDesktopCommands.Commands.Single( item => item.Area == CommandAreas.Install && item.Name == CommandNames.App );

        await Assert.ThrowsAsync<ToolException>( ( ) => command.Handler( context,
            ["--install-dir", destinationDirectory, "--replace-other", "--autostart", "none"] ) );

        Assert.Equal( "known-good-installation", File.ReadAllText( existingBinary ) );
        var request = Assert.Single( runner.Requests );
        Assert.Equal( Programs.Cargo, request.FileName );
        Assert.Equal( ["build", CommandLineOptions.Release, CommandLineOptions.Binaries], request.Arguments );
    }

    [Fact]
    public async Task UnapprovedInstallConflictIsRejectedBeforeBuild( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-install-conflict-test" );
        var existingBinary = Path.Combine( fixture.Path, ".local/bin", RepositoryNames.MainPackage );
        Directory.CreateDirectory( Path.GetDirectoryName( existingBinary )! );
        File.WriteAllText( existingBinary, "known-good-installation" );
        var destinationDirectory = Path.Combine( fixture.Path, "replacement", "bin" );
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.Home] = fixture.Path,
            [EnvironmentVariables.XdgConfigHome] = Path.Combine( fixture.Path, ".config" ),
        };
        var runner = new RejectingRunner( );
        var context = CreateContext( runner, name => environment.GetValueOrDefault( name ) );
        var command = NativeDesktopCommands.Commands.Single( item => item.Area == CommandAreas.Install && item.Name == CommandNames.App );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) => command.Handler( context,
            ["--install-dir", destinationDirectory, "--autostart", "none"] ) );

        Assert.Contains( "pass --replace-other", error.Message, StringComparison.Ordinal );
        Assert.Equal( "known-good-installation", File.ReadAllText( existingBinary ) );
        Assert.Empty( runner.Requests );
    }

    [Fact]
    public void DesktopExecPathPreservesBothEscapeLayers( )
    {
        const string path = """/tmp/configurator % install\root"quote`tick$cash/bin/wayscriber-configurator""";
        const string expected = """/tmp/configurator %% install\\\\root\\"quote\\`tick\\$cash/bin/wayscriber-configurator""";

        Assert.Equal( expected, NativeDesktopCommands.EscapeDesktopExecPath( path ) );
    }

    [Theory]
    [InlineData( "tools/test-package-repo-layout.sh" )]
    [InlineData( "tools/test-release-packaging.sh" )]
    [InlineData( "tools/test-aur-desktop-assets.sh" )]
    public async Task StandaloneReleaseContractsPassInTheCanonicalTestApp( string relativePath )
    {
        var root = FindRepository( );
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );

        var result = await runner.RunAsync(
            new ProcessRequest( Path.Combine( root, relativePath ), [], root, CaptureOutput: true, Trace: false ),
            CancellationToken.None );

        Assert.Equal( ExitCodes.Success, result.ExitCode );
    }

    private static ToolContext CreateContext( IProcessRunner runner, Func<string, string?>? environment = null ) =>
        new( FindRepository( ), TextWriter.Null, TextWriter.Null, runner, CancellationToken.None, environment );

    private static ToolCommand AurUpdateCommand( ) =>
        ReleaseAurCommands.Commands.Single( item => item.Area == "aur" && item.Name == "update" );

    private static string WriteManifest( string directory )
    {
        var path = Path.Combine( directory, "manifest.json" );
        File.WriteAllText( path, """{"version":"9.9.9","artifacts":[]}""" );
        return path;
    }

    private static void CreateSourceCheckout( string directory )
    {
        Directory.CreateDirectory( directory );
        File.WriteAllText( Path.Combine( directory, RepositoryNames.PackageBuildFile ), """
pkgname=wayscriber
pkgver=1.0.0
pkgrel=1
depends=(
    'gcc-libs'
    'wl-clipboard'
)
source=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums=('old')
package() {
    cd "$pkgname"
    install -Dm755 "target/release/wayscriber" "$pkgdir/usr/bin/wayscriber"
}
""" );
        File.WriteAllText( Path.Combine( directory, RepositoryNames.SourceInfoFile ), """
pkgbase = wayscriber
	pkgver = 1.0.0
	pkgrel = 1
	depends = gcc-libs
	depends = wl-clipboard
	source = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums = old

pkgname = wayscriber
""" );
    }

    private static void CreateConfiguratorCheckout( string directory )
    {
        Directory.CreateDirectory( directory );
        File.WriteAllText( Path.Combine( directory, RepositoryNames.PackageBuildFile ), """
pkgname=wayscriber-configurator
pkgver=1.0.0
pkgrel=1
pkgdesc='old'
depends=(
    'gcc-libs'
)
source=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums=('old')
package() {
    cd wayscriber
    install -Dm755 "target/release/wayscriber-configurator" "$pkgdir/usr/bin/wayscriber-configurator"
    install -Dm644 README.md "$pkgdir/usr/share/doc/wayscriber-configurator/README.md"
    install -Dm644 LICENSE "$pkgdir/usr/share/licenses/wayscriber-configurator/LICENSE"
}
""" );
        File.WriteAllText( Path.Combine( directory, RepositoryNames.SourceInfoFile ), """
pkgbase = wayscriber-configurator
	pkgver = 1.0.0
	pkgrel = 1
	pkgdesc = old
	depends = gcc-libs
	source = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums = old

pkgname = wayscriber-configurator
""" );
    }

    private static string FindRepository( )
    {
        var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
        return Path.GetFullPath( Path.Combine( directory, ".." ) );
    }

    private sealed class RejectingRunner : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class FailingBuildRunner : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            throw new ToolException( "Synthetic build failure." );
        }
    }

    private sealed class CheckoutOnlyRunner : IProcessRunner
    {
        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            if ( request.FileName == "git" && request.Arguments.Contains( "rev-parse" ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, Path.GetFullPath( request.Arguments[1] ) + "\n", string.Empty ) );
            }

            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class PublishingAurRunner : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.Arguments.Contains( "rev-parse" ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, Path.GetFullPath( request.Arguments[1] ) + "\n", string.Empty ) );
            }
            if ( request.Arguments.Contains( "ls-files" ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Failure, string.Empty, string.Empty ) );
            }
            if ( request.Arguments.Contains( "status" ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, " M PKGBUILD\n", string.Empty ) );
            }
            if ( request.FileName == "git" )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
            }

            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class AurSshRunner : IProcessRunner
    {
        public const string ScannedHostKey = "aur.archlinux.org ssh-ed25519 scanned-key\n";

        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName == "ssh-keyscan" && request.Arguments.SequenceEqual( new[] { "-H", "aur.archlinux.org" } ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, ScannedHostKey, string.Empty ) );
            }
            if ( request.FileName == "git" && request.Arguments.SequenceEqual( new[] { "ls-remote", "ssh://aur@aur.archlinux.org/wayscriber.git", "HEAD" } ) )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
            }

            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }
}
