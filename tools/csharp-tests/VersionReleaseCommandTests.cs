using System.Text.RegularExpressions;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class VersionReleaseCommandTests
{
    private const string FixtureChecksum = "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa";
    private const string OfflineFailure = "cannot resolve locked dependencies offline";
    private static readonly string[] VersionFiles =
        ["Cargo.toml", "configurator/Cargo.toml", "Cargo.lock", "packaging/PKGBUILD", "packaging/.SRCINFO"];

    // A version bump must retain a locked registry dependency even when a newer
    // compatible release exists. A local directory source makes this fully offline.
    [Fact]
    public async Task BumpRetainsLockedDependenciesAndKeepsHotfixesPackagingOnly( )
    {
        using var fixture = await CreateBumpFixture( );
        var lockBefore = fixture.Read( "Cargo.lock" );
        var runner = new CargoWithPackagingRunner( );

        Assert.Equal( ExitCodes.Success, await Bump( fixture, runner, "--dry-run", "1.0.1" ) );
        Assert.Equal( lockBefore, fixture.Read( "Cargo.lock" ) );
        Assert.Equal( "1.0.0", VersionCommands.ReadCargoVersion( fixture.PathFor( "Cargo.toml" ) ) );

        foreach ( var (release, cargo) in new[] { ("1.0.1", "1.0.1"), ("1.0.1.1", "1.0.1") } )
        {
            Assert.Equal( ExitCodes.Success, await Bump( fixture, runner, release ) );

            Assert.Equal( cargo, VersionCommands.ReadCargoVersion( fixture.PathFor( "Cargo.toml" ) ) );
            Assert.Equal( cargo, VersionCommands.ReadCargoVersion( fixture.PathFor( "configurator/Cargo.toml" ) ) );
            Assert.Contains( $"\npkgver={release}\n", fixture.Read( "packaging/PKGBUILD" ), StringComparison.Ordinal );
            Assert.Contains( $"\n\tpkgver = {release}\n", fixture.Read( "packaging/.SRCINFO" ), StringComparison.Ordinal );
            // Both workspace packages change, but all remaining bytes must stay put.
            var lockAfter = fixture.Read( "Cargo.lock" ).Replace( "version = \"1.0.1\"", "version = \"1.0.0\"", StringComparison.Ordinal );
            Assert.Equal( lockBefore, lockAfter );
        }

        // Prove the fixture exposes the original bug: resolving afresh selects 1.1.0.
        await RunCargo( fixture, "generate-lockfile", "--offline" );
        Assert.Contains( "name = \"release-fixture\"\nversion = \"1.1.0\"", fixture.Read( "Cargo.lock" ), StringComparison.Ordinal );
    }

    [Fact]
    public async Task BumpRollsBackEveryVersionFileWhenTheResultFailsTheVersionCheck( )
    {
        using var fixture = await CreateBumpFixture( );
        fixture.Replace( "README.md", "wayscriber?ref=RELEASE_TAG", "wayscriber?ref=v1.0.0" );
        var before = VersionFiles.Select( fixture.Read ).ToArray( );
        var runner = new CargoWithPackagingRunner( );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) => Bump( fixture, runner, "1.0.1" ) );

        Assert.StartsWith( "Version consistency check failed:", error.Message, StringComparison.Ordinal );
        Assert.Contains( "README.md: pinned flake ref 'wayscriber?ref=v1.0.0'", error.Message, StringComparison.Ordinal );
        Assert.Equal( before, VersionFiles.Select( fixture.Read ).ToArray( ) );
    }

    [Theory]
    [InlineData( true )]
    [InlineData( false )]
    public async Task BumpStopsBeforeEditingWhenLockedDependenciesCannotResolveOffline( bool dryRun )
    {
        using var fixture = await CreateBumpFixture( );
        Directory.Delete( fixture.PathFor( "vendor" ), recursive: true );
        Directory.CreateDirectory( fixture.PathFor( "vendor" ) );
        var before = VersionFiles.Select( fixture.Read ).ToArray( );
        var runner = new CargoWithPackagingRunner( );
        string[] arguments = dryRun ? ["--dry-run", "1.0.1"] : ["1.0.1"];

        var error = await Assert.ThrowsAsync<ToolException>( ( ) => Bump( fixture, runner, arguments ) );

        Assert.Contains( OfflineFailure, error.Message, StringComparison.Ordinal );
        Assert.Contains( "No version files changed.", error.Message, StringComparison.Ordinal );
        Assert.Equal( before, VersionFiles.Select( fixture.Read ).ToArray( ) );
        Assert.DoesNotContain( runner.Requests, request => request.FileName == Programs.Makepkg );
    }

    [Theory]
    [InlineData( CommandNames.CreateTag )]
    [InlineData( CommandNames.PublishTag )]
    public async Task ReleaseTagsValidateTheReleaseVersionBeforeTouchingGit( string command )
    {
        using var fixture = new VersionMetadataFixture( );
        var git = new GitRunner( );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) =>
            ReleaseCommand( command ).Handler( fixture.Context( git ), TagArguments( command, "99.98.97" ) ) );

        Assert.Contains( $"release version 99.98.97 must equal Cargo version {fixture.CargoVersion}", error.Message,
            StringComparison.Ordinal );
        Assert.Empty( git.Requests );
    }

    [Theory]
    [InlineData( CommandNames.CreateTag, " M Cargo.toml\n", false, "", "Working tree is not clean" )]
    [InlineData( CommandNames.PublishTag, "?? notes.txt\n", false, "", "Working tree is not clean" )]
    [InlineData( CommandNames.CreateTag, "", true, "", "already exists" )]
    [InlineData( CommandNames.PublishTag, "", true, "", "already exists locally" )]
    [InlineData( CommandNames.PublishTag, "", false, "0000000000000000000000000000000000000000\trefs/tags/{tag}\n",
        "already exists on origin" )]
    public async Task ReleaseTagsRefuseADirtyTreeOrAnExistingTag( string command, string status, bool localTag, string remoteTags,
        string expected )
    {
        using var fixture = new VersionMetadataFixture( );
        var tag = "v" + fixture.CargoVersion;
        var git = new GitRunner( status, localTag, remoteTags.Replace( "{tag}", tag, StringComparison.Ordinal ) );

        var error = await Assert.ThrowsAsync<ToolException>( ( ) =>
            ReleaseCommand( command ).Handler( fixture.Context( git ), TagArguments( command, fixture.CargoVersion ) ) );

        Assert.Contains( expected, error.Message, StringComparison.Ordinal );
        Assert.DoesNotContain( git.Requests, request => request.Arguments[0] is "tag" or "push" );
    }

    [Theory]
    [InlineData( CommandNames.CreateTag, false, true, false )]
    [InlineData( CommandNames.PublishTag, false, true, true )]
    [InlineData( CommandNames.PublishTag, true, false, false )]
    public async Task ReleaseTagsAcceptAPackagingHotfixRelease( string command, bool dryRun, bool tags, bool pushes )
    {
        using var fixture = new VersionMetadataFixture( );
        var release = fixture.CargoVersion + ".1";
        var tag = "v" + release;
        fixture.SetPackagingVersion( release );
        var git = new GitRunner( );
        using var output = new StringWriter( );
        string[] arguments = dryRun ? [.. TagArguments( command, release ), "--dry-run"] : TagArguments( command, release );

        Assert.Equal( ExitCodes.Success, await ReleaseCommand( command ).Handler( fixture.Context( git, output ), arguments ) );

        Assert.Equal( tags, git.Requests.Any( request => request.Arguments.SequenceEqual( ["tag", "-a", tag, "-m", $"Release {tag}"] ) ) );
        Assert.Equal( pushes, git.Requests.Any( request => request.Arguments.SequenceEqual( ["push", "origin", tag] ) ) );
        Assert.Equal( pushes, output.ToString( ).Contains( $"Pushing {tag} to origin", StringComparison.Ordinal ) );
        var announcesTag = command == CommandNames.PublishTag;
        Assert.Equal( announcesTag, output.ToString( ).Contains( $"Creating annotated tag {tag}", StringComparison.Ordinal ) );
    }

    // A branch named like the tag would make the pushed tag ambiguous, so publishing
    // looks the bare name up, which finds any local ref; creating a tag checks tags only.
    [Theory]
    [InlineData( CommandNames.PublishTag, "{tag}" )]
    [InlineData( CommandNames.CreateTag, "refs/tags/{tag}" )]
    public async Task ReleaseTagsLookUpTheExistingNameTheyWouldCollideWith( string command, string lookup )
    {
        using var fixture = new VersionMetadataFixture( );
        var tag = "v" + fixture.CargoVersion;
        var git = new GitRunner( localTag: true );

        await Assert.ThrowsAsync<ToolException>( ( ) =>
            ReleaseCommand( command ).Handler( fixture.Context( git ), TagArguments( command, fixture.CargoVersion ) ) );

        string[] expected = ["rev-parse", "-q", "--verify", lookup.Replace( "{tag}", tag, StringComparison.Ordinal )];
        Assert.Contains( git.Requests, request => request.Arguments.SequenceEqual( expected ) );
    }

    private static async Task<VersionMetadataFixture> CreateBumpFixture( )
    {
        var fixture = new VersionMetadataFixture( );
        try
        {
            await WriteCargoWorkspace( fixture );
            return fixture;
        }
        catch
        {
            fixture.Dispose( );
            throw;
        }
    }

    // The copied release metadata stays real; only the Cargo workspace is replaced by
    // a two-member workspace whose registry is a vendored directory source.
    private static async Task WriteCargoWorkspace( VersionMetadataFixture fixture )
    {
        fixture.Write( "Cargo.toml", """
[package]
name = "wayscriber"
version = "1.0.0"
edition = "2024"
[workspace]
members = ["configurator"]
[dependencies]
release-fixture = "1"

""" );
        fixture.Write( "configurator/Cargo.toml", """
[package]
name = "wayscriber-configurator"
version = "1.0.0"
edition = "2024"
[dependencies]
wayscriber = { path = ".." }
libadwaita = { version = "0.9", features = ["v1_4"] }

""" );
        fixture.Write( "src/lib.rs", string.Empty );
        fixture.Write( "configurator/src/lib.rs", string.Empty );
        fixture.Write( ".cargo/config.toml", $"""
[source.crates-io]
replace-with = "fixture"
[source.fixture]
directory = "{fixture.PathFor( "vendor" )}"

""" );
        WriteVendoredCrate( fixture, "release-fixture", "1.0.0", string.Empty );
        WriteVendoredCrate( fixture, "libadwaita", "0.9.0", "[features]\nv1_4 = []\n" );
        fixture.SetPackagingVersion( "1.0.0" );
        await RunCargo( fixture, "generate-lockfile", "--offline" );

        // Vendored only after locking, so a fresh resolution would now select it.
        WriteVendoredCrate( fixture, "release-fixture", "1.1.0", string.Empty );
    }

    private static void WriteVendoredCrate( VersionMetadataFixture fixture, string name, string version, string features )
    {
        var directory = $"vendor/{name}-{version}";
        var manifest = $"[package]\nname = \"{name}\"\nversion = \"{version}\"\nedition = \"2024\"\n{features}";
        fixture.Write( $"{directory}/Cargo.toml", manifest );
        fixture.Write( $"{directory}/src/lib.rs", string.Empty );
        fixture.Write( $"{directory}/.cargo-checksum.json", $$"""{"files":{},"package":"{{FixtureChecksum}}"}""" );
    }

    private static Task<ProcessResult> RunCargo( VersionMetadataFixture fixture, params string[] arguments ) =>
        new ProcessRunner( TextWriter.Null, TextWriter.Null ).RunAsync(
            new ProcessRequest( Programs.Cargo, arguments, fixture.Root, CaptureOutput: true, Trace: false ), CancellationToken.None );

    private static Task<int> Bump( VersionMetadataFixture fixture, IProcessRunner runner, params string[] arguments ) =>
        VersionMetadataFixture.VersionCommand( CommandNames.Bump ).Handler( fixture.Context( runner ), arguments );

    private static ToolCommand ReleaseCommand( string name ) =>
        ReleaseAurCommands.Commands.Single( command => command.Area == CommandAreas.Release && command.Name == name );

    private static string[] TagArguments( string command, string version ) =>
        command == CommandNames.PublishTag ? [CommandLineOptions.Version, version] : [version];

    // Runs the real Cargo against the fixture and stands in for makepkg by
    // regenerating the copied .SRCINFO for the PKGBUILD's new pkgver.
    private sealed class CargoWithPackagingRunner : IProcessRunner
    {
        private readonly ProcessRunner _processes = new( TextWriter.Null, TextWriter.Null );

        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName == Programs.Cargo )
            {
                return _processes.RunAsync( request with { CaptureOutput = true, Trace = false }, cancellationToken );
            }
            if ( request.FileName == Programs.Makepkg && request.Arguments is ["--printsrcinfo"] )
            {
                var packageBuild = File.ReadAllText( Path.Combine( request.WorkingDirectory, RepositoryNames.PackageBuildFile ) );
                var sourceInfo = File.ReadAllText( Path.Combine( request.WorkingDirectory, RepositoryNames.SourceInfoFile ) );
                var previous = Regex.Match( sourceInfo, "(?m)^\tpkgver = (.+)$" ).Groups[1].Value;
                var next = Regex.Match( packageBuild, "(?m)^pkgver=(.+)$" ).Groups[1].Value;
                var regenerated = sourceInfo.Replace( previous, next, StringComparison.Ordinal );
                return Task.FromResult( new ProcessResult( ExitCodes.Success, regenerated, string.Empty ) );
            }

            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }

    private sealed class GitRunner( string status = "", bool localTag = false, string remoteTags = "" ) : IProcessRunner
    {
        private static readonly string[] KnownCommands = ["status", "rev-parse", "ls-remote", "tag", "push"];

        public List<ProcessRequest> Requests { get; } = [];

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Requests.Add( request );
            if ( request.FileName != Programs.Git || !KnownCommands.Contains( request.Arguments[0] ) )
            {
                throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
            }

            var exitCode = request.Arguments[0] == "rev-parse" && !localTag ? ExitCodes.Failure : ExitCodes.Success;
            var output = request.Arguments[0] switch
            {
                "status" => status,
                "ls-remote" => remoteTags,
                _ => string.Empty,
            };
            return Task.FromResult( new ProcessResult( exitCode, output, string.Empty ) );
        }
    }
}
