using System.Text.RegularExpressions;
using Xunit;

namespace Wayscriber.Tools.Tests;

// A temporary repository holding a copy of every file the version checker reads.
// The packaging recipes start at the Cargo version so mutations have a fixed baseline.
internal sealed class VersionMetadataFixture : IDisposable
{
    private static readonly string[] MetadataFiles =
    [
        "Cargo.toml", "Cargo.lock", "README.md", "flake.nix", "global.json", "configurator/Cargo.toml",
        "packaging/PKGBUILD", "packaging/.SRCINFO", "packaging/package.configurator.yaml",
        ".github/workflows/build-packages.yml", "tools/update-aur-from-manifest.sh",
    ];

    private readonly TemporaryDirectory _directory = new( "wayscriber-version-metadata-test" );

    public VersionMetadataFixture( )
    {
        var repository = TestRepository.Root;
        foreach ( var relativePath in MetadataFiles )
        {
            var destination = PathFor( relativePath );
            Directory.CreateDirectory( Path.GetDirectoryName( destination )! );
            File.Copy( Path.Combine( [repository, .. relativePath.Split( '/' )] ), destination );
        }

        CargoVersion = VersionCommands.ReadCargoVersion( PathFor( "Cargo.toml" ) );
        SetPackagingVersion( CargoVersion );
    }

    public string Root => _directory.Path;

    public string CargoVersion
    {
        get;
    }

    public string PathFor( string relativePath ) => Path.Combine( [Root, .. relativePath.Split( '/' )] );

    public string Read( string relativePath ) => File.ReadAllText( PathFor( relativePath ) );

    public void Write( string relativePath, string content )
    {
        Directory.CreateDirectory( Path.GetDirectoryName( PathFor( relativePath ) )! );
        File.WriteAllText( PathFor( relativePath ), content );
    }

    // Replaces every occurrence; the current text must exist so a stale fixture cannot pass vacuously.
    public void Replace( string relativePath, string current, string replacement )
    {
        var text = Read( relativePath );
        Assert.Contains( current, text, StringComparison.Ordinal );
        Write( relativePath, text.Replace( current, replacement, StringComparison.Ordinal ) );
    }

    public void SetPackagingVersion( string version )
    {
        Write( "packaging/PKGBUILD", Regex.Replace( Read( "packaging/PKGBUILD" ), "(?m)^pkgver=.*$", $"pkgver={version}" ) );
        Write( "packaging/.SRCINFO", Regex.Replace( Read( "packaging/.SRCINFO" ), "(?m)^\tpkgver = .*$", $"\tpkgver = {version}" ) );
    }

    public ToolContext Context( IProcessRunner runner, TextWriter? output = null ) =>
        new( Root, output ?? TextWriter.Null, TextWriter.Null, runner, CancellationToken.None );

    public async Task<string> Check( params string[] arguments )
    {
        using var output = new StringWriter( );

        var exitCode = await VersionCommand( CommandNames.Check ).Handler( Context( new NoProcessRunner( ), output ), arguments );

        Assert.Equal( ExitCodes.Success, exitCode );

        return output.ToString( );
    }

    public async Task<string> CheckExpectingFailure( params string[] arguments )
    {
        var error = await Assert.ThrowsAsync<ToolException>( ( ) =>
            VersionCommand( CommandNames.Check ).Handler( Context( new NoProcessRunner( ) ), arguments ) );

        Assert.StartsWith( "Version consistency check failed:\n", error.Message, StringComparison.Ordinal );
        return error.Message;
    }

    public static ToolCommand VersionCommand( string name ) =>
        VersionCommands.Commands.Single( command => command.Area == CommandAreas.Version && command.Name == name );

    public void Dispose( ) => _directory.Dispose( );

    // Metadata checks read files only; any process launch is a regression.
    private sealed class NoProcessRunner : IProcessRunner
    {
        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            throw new Xunit.Sdk.XunitException( $"Unexpected process: {request.FileName} {string.Join( ' ', request.Arguments )}" );
        }
    }
}
