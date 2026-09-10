using System.Runtime.Versioning;
using System.Text;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class SignedRepositoryIntegrationTests
{
    private const string Architecture = "x86_64";
    private const string KeyIdentity = "Wayscriber repository test <repository-test@wayscriber.invalid>";
    private const string KeyPassphrase = "wayscriber-repository-test-passphrase";
    private const string PackageVersion = "1.0.0";

    [Fact]
    public async Task RepositoryBuildProducesAVerifiableSignedRpm( )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = new TemporaryDirectory( "wayscriber-signed-repository-test" );
        var artifacts = Path.Combine( fixture.Path, "artifacts" );
        var output = Path.Combine( fixture.Path, "repository" );
        Directory.CreateDirectory( artifacts );
        await CreateDebPackage( fixture.Path, artifacts );
        await CreateRpmPackage( fixture.Path, artifacts );
        var privateKey = await CreateSigningKey( fixture.Path );
        var environment = new Dictionary<string, string?>( StringComparer.Ordinal )
        {
            [EnvironmentVariables.GpgPrivateKeyBase64] = Convert.ToBase64String( Encoding.UTF8.GetBytes( privateKey ) ),
            [EnvironmentVariables.GpgPassphrase] = KeyPassphrase,
            [EnvironmentVariables.SignRpms] = "1",
        };
        var context = new ToolContext( FindRepository( ), TextWriter.Null, TextWriter.Null,
            new ProcessRunner( TextWriter.Null, TextWriter.Null ), CancellationToken.None,
            name => environment.GetValueOrDefault( name ) );
        var command = PackagingCommands.Commands.Single( item => item.Area == "package" && item.Name == "build-repositories" );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, ["--artifact-root", artifacts, "--output-root", output] ) );

        var rpmDatabase = Path.Combine( fixture.Path, "rpm-database" );
        Directory.CreateDirectory( rpmDatabase );
        var publicKey = Path.Combine( output, "rpm/RPM-GPG-KEY-wayscriber.asc" );
        var signedRpm = Path.Combine( output, $"rpm/{RepositoryNames.MainPackage}-{Architecture}.rpm" );
        await Run( fixture.Path, "rpmkeys", ["--dbpath", rpmDatabase, "--import", publicKey] );
        var verification = await Run( fixture.Path, "rpmkeys", ["--dbpath", rpmDatabase, "--checksig", signedRpm],
            allowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } );

        Assert.True( verification.IsSuccess, verification.StandardOutput + verification.StandardError );
        Assert.Contains( "digests signatures OK", verification.StandardOutput, StringComparison.OrdinalIgnoreCase );
    }

    private static async Task CreateDebPackage( string root, string artifacts )
    {
        var packageRoot = Path.Combine( root, "deb-package" );
        var metadata = Path.Combine( packageRoot, "DEBIAN" );
        Directory.CreateDirectory( metadata );
        File.WriteAllText( Path.Combine( metadata, "control" ), $"""
Package: wayscriber
Version: {PackageVersion}
Architecture: amd64
Maintainer: Wayscriber Tests <repository-test@wayscriber.invalid>
Description: Wayscriber repository signing fixture

""" );
        Directory.CreateDirectory( Path.Combine( packageRoot, "usr/share/wayscriber" ) );
        File.WriteAllText( Path.Combine( packageRoot, "usr/share/wayscriber/fixture" ), "fixture\n" );
        await Run( root, "dpkg-deb", ["--build", packageRoot, Path.Combine( artifacts, $"{RepositoryNames.MainPackage}-amd64.deb" )] );
    }

    private static async Task CreateRpmPackage( string root, string artifacts )
    {
        var topDirectory = Path.Combine( root, "rpmbuild" );
        var specDirectory = Path.Combine( topDirectory, "SPECS" );
        Directory.CreateDirectory( specDirectory );
        var spec = Path.Combine( specDirectory, "wayscriber.spec" );
        File.WriteAllText( spec, $$"""
Name: wayscriber
Version: {{PackageVersion}}
Release: 1
Summary: Wayscriber repository signing fixture
License: MIT
BuildArch: {{Architecture}}

%description
Wayscriber repository signing fixture.

%install
mkdir -p %{buildroot}/usr/share/wayscriber
echo fixture > %{buildroot}/usr/share/wayscriber/fixture

%files
/usr/share/wayscriber/fixture

""" );
        await Run( root, "rpmbuild", ["--define", $"_topdir {topDirectory}", "--define", "_build_id_links none", "-bb", spec] );
        var builtRpm = Directory.EnumerateFiles( Path.Combine( topDirectory, "RPMS" ), "*.rpm", SearchOption.AllDirectories ).Single( );
        File.Copy( builtRpm, Path.Combine( artifacts, $"{RepositoryNames.MainPackage}-{Architecture}.rpm" ) );
    }

    [SupportedOSPlatform( "linux" )]
    private static async Task<string> CreateSigningKey( string root )
    {
        var home = Path.Combine( root, "signing-key" );
        Directory.CreateDirectory( home );
        File.SetUnixFileMode( home, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
        var environment = new Dictionary<string, string?> { [EnvironmentVariables.GnuPgHome] = home };
        await Run( root, "gpg", ["--batch", "--pinentry-mode", "loopback", "--passphrase", KeyPassphrase,
            "--quick-generate-key", KeyIdentity, "rsa2048", "sign", "0"], environment );
        var exported = await Run( root, "gpg", ["--batch", "--pinentry-mode", "loopback", "--passphrase", KeyPassphrase,
            "--armor", "--export-secret-keys", KeyIdentity], environment );
        return exported.StandardOutput;
    }

    private static Task<ProcessResult> Run( string workingDirectory, string fileName, IReadOnlyList<string> arguments,
        IReadOnlyDictionary<string, string?>? environment = null, IReadOnlySet<int>? allowedExitCodes = null )
    {
        var runner = new ProcessRunner( TextWriter.Null, TextWriter.Null );
        var request = new ProcessRequest( fileName, arguments, workingDirectory, environment, CaptureOutput: true,
            Trace: false, AllowedExitCodes: allowedExitCodes );
        return runner.RunAsync( request, CancellationToken.None );
    }

    private static string FindRepository( )
    {
        var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
        return Path.GetFullPath( Path.Combine( directory, ".." ) );
    }
}
