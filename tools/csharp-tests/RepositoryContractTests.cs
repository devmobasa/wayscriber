using System.Text.Json;
using System.Text.RegularExpressions;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class RepositoryContractTests
{
    [Fact]
    public void StandaloneInstallersRemainAvailableWithoutDotnet( )
    {
        var root = FindRepository( );
        Assert.True( File.Exists( Path.Combine( root, "tools/install.sh" ) ) );
        Assert.True( File.Exists( Path.Combine( root, "tools/install-configurator.sh" ) ) );
        Assert.DoesNotContain( "dotnet", File.ReadAllText( Path.Combine( root, "tools/install.sh" ) ), StringComparison.OrdinalIgnoreCase );
        Assert.DoesNotContain( "dotnet", File.ReadAllText( Path.Combine( root, "tools/install-configurator.sh" ) ), StringComparison.OrdinalIgnoreCase );
    }

    [Fact]
    public void StandaloneDevelopmentRunnerUsesTheBuiltBinary( )
    {
        var source = File.ReadAllText( Path.Combine( FindRepository( ), "tools/run.sh" ) );

        Assert.Contains( "target/release/wayscriber", source, StringComparison.Ordinal );
        Assert.Contains( "--daemon", source, StringComparison.Ordinal );
        Assert.Contains( "RUST_LOG=\"${RUST_LOG:-info}\"", source, StringComparison.Ordinal );
        Assert.DoesNotContain( "dotnet", source, StringComparison.OrdinalIgnoreCase );
    }

    [Fact]
    public void CsharpInstallShortcutRoutesToTheSharedAppInstaller( )
    {
        var source = File.ReadAllText( Path.Combine( FindRepository( ), "tools/install.cs" ) );

        Assert.StartsWith( "#!/usr/bin/env -S dotnet run --disable-build-servers --file\n", source, StringComparison.Ordinal );
        Assert.Contains( "#:include csharp/includes.cs", source, StringComparison.Ordinal );
        Assert.Contains( "var command = explicitCommand ? args[0] : CommandNames.App;", source, StringComparison.Ordinal );
        Assert.Contains( "ToolApplication.RunAsync( [CommandAreas.Install, command, .. commandArguments] )", source, StringComparison.Ordinal );
        Assert.DoesNotContain( "install.sh", source, StringComparison.Ordinal );
    }

    [Fact]
    public void StandaloneShellToolsDoNotRedirectToDotnet( )
    {
        var root = Path.Combine( FindRepository( ), "tools" );
        foreach ( var path in Directory.EnumerateFiles( root, "*.sh", SearchOption.TopDirectoryOnly ) )
        {
            var source = File.ReadAllText( path );
            Assert.DoesNotContain( "dotnet run tools/wayscriber.cs", source, StringComparison.OrdinalIgnoreCase );
            Assert.DoesNotMatch( @"(?m)^\s*(?:exec\s+)?dotnet\b", source );
        }
    }

    [Fact]
    public void WorkflowRunStepsUseTheCsharpEntryPoint( )
    {
        foreach ( var path in new[] { ".github/workflows/ci.yml", ".github/workflows/build-packages.yml" } )
        {
            var text = File.ReadAllText( Path.Combine( FindRepository( ), path ) );
            var commands = Regex.Matches( text, @"(?m)^\s+run:\s*(.+)$" ).Select( match => match.Groups[1].Value.Trim( ) ).ToArray( );
            Assert.NotEmpty( commands );
            Assert.All( commands, command => Assert.StartsWith( "dotnet ", command ) );
            Assert.DoesNotContain( "bash", text, StringComparison.Ordinal );
            Assert.DoesNotContain( "python", text, StringComparison.Ordinal );
        }
    }

    [Fact]
    public void CanonicalCsharpGateEnforcesCsharpFormatting( )
    {
        var source = File.ReadAllText( Path.Combine( FindRepository( ), "tools/csharp/Commands/DevelopmentCommands.cs" ) );

        Assert.Contains( "CsharpFileApps", source, StringComparison.Ordinal );
        Assert.Contains( "CsharpFormatModes", source, StringComparison.Ordinal );
        Assert.Contains( "RunCsharpFormattingChecks( context )", source, StringComparison.Ordinal );
        Assert.Contains( "CommandLineOptions.VerifyNoChanges", source, StringComparison.Ordinal );
    }

    [Fact]
    public void CancellationDiagnosticDoesNotReuseTheCanceledToken( )
    {
        var source = File.ReadAllText( Path.Combine( FindRepository( ), "tools/csharp/Application/ToolApplication.cs" ) );

        Assert.Contains( "WriteLineAsync( ToolMessages.Canceled );", source, StringComparison.Ordinal );
        Assert.DoesNotContain( "WriteLineAsync( ToolMessages.Canceled, cancellation.Token )", source, StringComparison.Ordinal );
    }

    [Fact]
    public void ToolModulesAreExplicitlyIncluded( )
    {
        var root = FindRepository( );
        var csharpRoot = Path.Combine( root, "tools/csharp" );
        var actual = Directory.EnumerateFiles( csharpRoot, "*.cs", SearchOption.AllDirectories )
            .Where( path => Path.GetFileName( path ) != "includes.cs" )
            .Select( path => Path.GetRelativePath( csharpRoot, path ).Replace( Path.DirectorySeparatorChar, '/' ) ).Order( ).ToArray( );
        var included = File.ReadAllLines( Path.Combine( csharpRoot, "includes.cs" ) )
            .Select( line => Regex.Match( line, @"^#:include (.+)$" ) ).Where( match => match.Success )
            .Select( match => match.Groups[1].Value ).Order( ).ToArray( );
        Assert.Equal( actual, included );
    }

    [Fact]
    public void ToolSdkIsPinnedInGlobalJson( )
    {
        var root = FindRepository( );
        using var document = JsonDocument.Parse( File.ReadAllText( Path.Combine( root, "global.json" ) ) );
        var sdk = document.RootElement.GetProperty( "sdk" );
        Assert.Equal( VersionCommands.ToolSdkVersion, sdk.GetProperty( "version" ).GetString( ) );
        Assert.Equal( VersionCommands.ToolSdkRollForward, sdk.GetProperty( "rollForward" ).GetString( ) );
        Assert.True( sdk.GetProperty( "allowPrerelease" ).GetBoolean( ) );
    }

    [Fact]
    public void CsharpCommandsNeverLaunchAShellOrPythonInterpreter( )
    {
        var root = Path.Combine( FindRepository( ), "tools/csharp" );
        foreach ( var path in Directory.EnumerateFiles( root, "*.cs", SearchOption.AllDirectories ) )
        {
            var source = File.ReadAllText( path );
            Assert.DoesNotMatch( "(?:context\\.Run|ProcessRequest)\\(\\s*\"(?:(?:ba|z)?sh|python(?:3(?:\\.\\d+)?)?)\"", source );
        }
    }

    [Fact]
    public void CsharpProcessAndEnvironmentContractsUseNamedConstants( )
    {
        var root = Path.Combine( FindRepository( ), "tools/csharp" );
        foreach ( var path in Directory.EnumerateFiles( root, "*.cs", SearchOption.AllDirectories )
                     .Where( path => Path.GetFileName( path ) != "ToolConstants.cs" ) )
        {
            var source = File.ReadAllText( path );
            Assert.DoesNotMatch( "(?:context\\.Run|ProcessRequest|ProcessStartInfo)\\(\\s*\\\"", source );
            Assert.DoesNotMatch( "context\\.Environment\\(\\s*\\\"[A-Z][A-Z0-9_]+\\\"", source );
            Assert.DoesNotMatch( "ToolException\\([^\\n]*,\\s*(?:2|70|127|130)\\s*\\)", source );
        }
    }

    [Fact]
    public async Task ProcessRunnerPreservesArgumentBoundaries( )
    {
        var output = new StringWriter( );
        var error = new StringWriter( );
        var result = await new ProcessRunner( output, error ).RunAsync(
            new ProcessRequest( "/usr/bin/printf", ["%s", "two words;$(ignored)"], FindRepository( ), CaptureOutput: true ), CancellationToken.None );
        Assert.Equal( "two words;$(ignored)", result.StandardOutput );
    }

    [Fact]
    public async Task ProcessRunnerAllowsDeclaredNonzeroExit( )
    {
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );
        var result = await runner.RunAsync( new ProcessRequest( "/usr/bin/false", [], FindRepository( ), AllowedExitCodes: new HashSet<int> { ExitCodes.Failure } ), CancellationToken.None );
        Assert.Equal( ExitCodes.Failure, result.ExitCode );
    }

    [Fact]
    public async Task ConcurrentProcessRunsDoNotShareALock( )
    {
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );
        var runs = Enumerable.Range( 0, 12 ).Select( value => runner.RunAsync(
            new ProcessRequest( "/usr/bin/printf", ["%s", value.ToString( )], FindRepository( ), CaptureOutput: true ), CancellationToken.None ) );
        var results = await Task.WhenAll( runs );
        Assert.Equal( Enumerable.Range( 0, 12 ).Select( value => value.ToString( ) ).Order( ), results.Select( result => result.StandardOutput ).Order( ) );
    }

    [Fact]
    public void AssetRecipesAreDeterministicUnderConcurrency( )
    {
        var values = Enumerable.Range( 0, 16 ).AsParallel( ).Select( _ => AssetsCommand.CreateRecipe( FindRepository( ) ).ToJsonString( ) ).ToArray( );
        Assert.Single( values.Distinct( StringComparer.Ordinal ) );
    }

    [Fact]
    public async Task StandaloneAndCsharpAssetRecipesHaveExactParity( )
    {
        var root = FindRepository( );
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );
        var result = await runner.RunAsync( new ProcessRequest( "/usr/bin/bash", [Path.Combine( root, "tools/aur-desktop-assets.sh" ), root],
            root, CaptureOutput: true, Trace: false ), CancellationToken.None );
        Assert.Equal( AssetsCommand.CreateRecipe( root ).ToJsonString( ), result.StandardOutput.Trim( ) );
    }

    [Fact]
    public async Task AurSourceUpdateDoesNotNeedMakepkgAndIsIdempotent( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-aur-test" );
        var source = Path.Combine( fixture.Path, "source" );
        Directory.CreateDirectory( source );
        File.WriteAllText( Path.Combine( source, "PKGBUILD" ), """
pkgname=wayscriber
pkgver=1.0.0
pkgrel=7
install=wayscriber.install
depends=(
    'gcc-libs'
    'wl-clipboard'
)
makedepends=(
    'git'
)
source=("old.tar.gz::https://example.invalid/old.tar.gz")
sha256sums=('old')
package() {
    cd "$pkgname"
    install -Dm755 "target/release/wayscriber" "$pkgdir/usr/bin/wayscriber"
}
""" );
        File.WriteAllText( Path.Combine( source, ".SRCINFO" ), """
pkgbase = wayscriber
	pkgver = 1.0.0
	pkgrel = 7
	install = wayscriber.install
	makedepends = git
	depends = gcc-libs
	depends = wl-clipboard
	source = old.tar.gz::https://example.invalid/old.tar.gz
	sha256sums = old

pkgname = wayscriber
""" );
        File.WriteAllText( Path.Combine( source, "wayscriber.install" ), "obsolete" );
        var manifest = Path.Combine( fixture.Path, "manifest.json" );
        File.WriteAllText( manifest, """{"version":"9.9.9","artifacts":[]}""" );
        var runner = new RecordingAurRunner( );
        var context = new ToolContext( FindRepository( ), TextWriter.Null, TextWriter.Null, runner, CancellationToken.None );
        var command = ReleaseAurCommands.Commands.Single( item => item.Area == "aur" && item.Name == "update" );
        var arguments = new[] { "--manifest", manifest, "--source-dir", source, "--bin-dir", Path.Combine( fixture.Path, "missing" ),
            "--no-configurator", "--source-sha256", new string( 'a', HashingConstants.Sha256HexLength ) };

        Assert.Equal( ExitCodes.Success, await command.Handler( context, arguments ) );
        Assert.Equal( ExitCodes.Success, await command.Handler( context, arguments ) );

        var pkgbuild = File.ReadAllText( Path.Combine( source, "PKGBUILD" ) );
        var srcinfo = File.ReadAllText( Path.Combine( source, ".SRCINFO" ) );
        Assert.Contains( "pkgver=9.9.9", pkgbuild );
        Assert.Contains( "pkgrel=2", pkgbuild );
        Assert.Contains( "# End Wayscriber desktop integration", pkgbuild );
        Assert.Single( Regex.Matches( pkgbuild, "# Wayscriber desktop integration" ).Cast<Match>( ) );
        Assert.Contains( "\tpkgver = 9.9.9", srcinfo );
        Assert.Contains( "\tpkgrel = 2", srcinfo );
        Assert.Contains( "\tdepends = gtk4-layer-shell", srcinfo );
        Assert.DoesNotContain( "makedepends = git", srcinfo );
        Assert.DoesNotContain( "install = ", srcinfo );
        Assert.False( File.Exists( Path.Combine( source, "wayscriber.install" ) ) );
        Assert.DoesNotContain( runner.Requests, request => request.FileName is "makepkg" or "bash" or "sh" );
    }

    [Fact]
    public async Task RepositoryBuildReplacesCompleteOutputFromAStagingDirectory( )
    {
        using var fixture = new TemporaryDirectory( "wayscriber-repository-test" );
        var artifacts = Path.Combine( fixture.Path, "artifacts" );
        var output = Path.Combine( fixture.Path, "repository" );
        Directory.CreateDirectory( artifacts );
        Directory.CreateDirectory( output );
        File.WriteAllText( Path.Combine( output, "stale" ), "old" );
        foreach ( var name in new[] { "wayscriber-amd64.deb", "wayscriber-configurator-amd64.deb", "wayscriber-x86_64.rpm", "wayscriber-configurator-x86_64.rpm" } )
        {
            File.WriteAllText( Path.Combine( artifacts, name ), name );
        }
        var runner = new RepositoryLayoutRunner( );
        var context = new ToolContext( FindRepository( ), TextWriter.Null, TextWriter.Null, runner, CancellationToken.None );
        var command = PackagingCommands.Commands.Single( item => item.Area == "package" && item.Name == "build-repositories" );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, ["--artifact-root", artifacts, "--output-root", output] ) );

        Assert.False( File.Exists( Path.Combine( output, "stale" ) ) );
        Assert.True( File.Exists( Path.Combine( output, "apt/dists/stable/main/binary-amd64/Packages" ) ) );
        Assert.True( File.Exists( Path.Combine( output, "apt/dists/stable/main/binary-amd64/Packages.gz" ) ) );
        Assert.True( File.Exists( Path.Combine( output, "rpm/repodata/repomd.xml" ) ) );
        Assert.DoesNotContain( runner.Requests, request => request.FileName is "bash" or "sh" );
    }

    [Theory]
    [InlineData( "mode: 493", "mode: nope" )]
    [InlineData( "mode: 493", "mode: '493'" )]
    public void AssetRecipesRejectMalformedModes( string oldValue, string newValue )
    {
        using var fixture = AssetFixture( );
        var path = Path.Combine( fixture.Path, "packaging/package.wayscriber.yaml" );
        File.WriteAllText( path, File.ReadAllText( path ).Replace( oldValue, newValue, StringComparison.Ordinal ) );
        Assert.Throws<ToolException>( ( ) => AssetsCommand.CreateRecipe( fixture.Path ) );
    }

    [Theory]
    [InlineData( "'0644 ../escape'" )]
    [InlineData( "'0644 bin/wayscriber' \\\n'0644 bin/wayscriber'" )]
    public void InstallerManifestRejectsUnsafeEntries( string entry )
    {
        var source = $"# ARCH_INSTALL_MANIFEST_BEGIN\nrelease_manifest() {{\nprintf '%s\\n' \\\n{entry}\n}}\n# ARCH_INSTALL_MANIFEST_END\n";
        Assert.Throws<ToolException>( ( ) => PackagingCommands.ParseInstallerManifest( source ) );
    }

    [Fact]
    public void VersionCheckAcceptsPackagingHotfixOfCurrentCargoVersion( )
    {
        var current = ReleaseVersion.Parse( VersionCommands.ReadCargoVersion( Path.Combine( FindRepository( ), "Cargo.toml" ) ) );
        var packageVersion = File.ReadAllText( Path.Combine( FindRepository( ), "packaging/PKGBUILD" ) );
        var currentPackage = Regex.Match( packageVersion, @"(?m)^pkgver=(.+)$" ).Groups[1].Value;
        var release = currentPackage.StartsWith( current.CargoVersion + ".", StringComparison.Ordinal ) ? currentPackage : current.CargoVersion;
        Assert.Empty( VersionCommands.Validate( FindRepository( ), release ) );
    }

    [Fact]
    public void VersionCheckRejectsUnrelatedReleaseVersion( )
    {
        var errors = VersionCommands.Validate( FindRepository( ), "99.98.97" );
        Assert.Contains( errors, error => error.Contains( "must equal Cargo version", StringComparison.Ordinal ) );
    }

    [Fact]
    public void ReplaceSingleRejectsAmbiguousMutation( )
    {
        Assert.Throws<ToolException>( ( ) => Files.ReplaceSingle( "x x", "x", "y", "fixture" ) );
    }

    [Fact]
    public void AtomicFileSetRestoresEarlierFilesWhenCommitFails( )
    {
        using var directory = new TemporaryDirectory( "wayscriber-atomic-test" );
        var first = Path.Combine( directory.Path, "first" );
        var blocker = Path.Combine( directory.Path, "blocker" );
        File.WriteAllText( first, "before" );
        File.WriteAllText( blocker, "file" );
        var files = new AtomicFileSet( );
        files.Add( first, "after" );
        files.Add( Path.Combine( blocker, "child" ), "never" );
        Assert.ThrowsAny<IOException>( files.Commit );
        Assert.Equal( "before", File.ReadAllText( first ) );
    }

    private static TemporaryDirectory AssetFixture( )
    {
        var fixture = new TemporaryDirectory( "wayscriber-assets-test" );
        Directory.CreateDirectory( Path.Combine( fixture.Path, "packaging" ) );
        foreach ( var name in new[] { "package.wayscriber.yaml", "package.configurator.yaml" } )
        {
            File.Copy( Path.Combine( FindRepository( ), "packaging", name ), Path.Combine( fixture.Path, "packaging", name ) );
        }
        return fixture;
    }

    private static string FindRepository( )
    {
        var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
        return Path.GetFullPath( Path.Combine( directory, ".." ) );
    }

    private sealed class RecordingAurRunner : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName == "git" && request.Arguments.Count >= 3 && request.Arguments[0] == "-C" && request.Arguments[2] == "rev-parse" )
            {
                return Task.FromResult( new ProcessResult( ExitCodes.Success, Path.GetFullPath( request.Arguments[1] ) + "\n", string.Empty ) );
            }
            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class RepositoryLayoutRunner : IProcessRunner
    {
        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName == "apt-ftparchive" )
            {
                var output = request.Arguments.Contains( "packages" ) ? "Package: wayscriber\n" : "Suite: stable\n";
                return Task.FromResult( new ProcessResult( ExitCodes.Success, output, string.Empty ) );
            }
            if ( request.FileName == "createrepo_c" )
            {
                var root = request.Arguments[^1];
                Directory.CreateDirectory( Path.Combine( root, "repodata" ) );
                File.WriteAllText( Path.Combine( root, "repodata/repomd.xml" ), "<repomd/>" );
                return Task.FromResult( new ProcessResult( ExitCodes.Success, string.Empty, string.Empty ) );
            }
            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }
}
