using System.Text;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class CoreTests
{
    // The exit code git uses for a fatal error, such as running outside a repository.
    private const int GitFatalExitCode = 128;

    [Theory]
    [InlineData( "1.2.3", "1.2.3", false )]
    [InlineData( "1.2.3.4", "1.2.3", true )]
    public void ReleaseVersionParses( string value, string cargo, bool hotfix )
    {
        var version = ReleaseVersion.Parse( value );
        Assert.Equal( cargo, version.CargoVersion );
        Assert.Equal( hotfix, version.IsHotfix );
        Assert.Equal( value, version.ToString( ) );
    }

    [Theory]
    [InlineData( "1.2" )]
    [InlineData( "01.2.3" )]
    [InlineData( "1.2.3\n" )]
    [InlineData( "1.2.3.04" )]
    [InlineData( "99999999999.0.0" )]
    public void ReleaseVersionRejectsMalformedInput( string value ) =>
        Assert.Throws<ToolException>( ( ) => ReleaseVersion.Parse( value ) );

    [Theory]
    [InlineData( "wayscriber-v1.2.3-linux-x86_64", true )]
    [InlineData( "wayscriber-v1.2.3.1-linux-x86_64", true )]
    [InlineData( "wayscriber-v01.2.3-linux-x86_64", false )]
    [InlineData( "wayscriber-v1.2-linux-x86_64", false )]
    [InlineData( "wayscriber-v-linux-x86_64", false )]
    [InlineData( "wayscriber-1.2.3-linux-x86_64", false )]
    [InlineData( "wayscriber-v1.2.3-linux-aarch64", false )]
    public void ReleaseArchiveRootsNameAReleaseVersion( string root, bool accepted ) =>
        Assert.Equal( accepted, PackagingCommands.IsReleaseArchiveRoot( root ) );

    [Fact]
    public void NextPatchReportsAnExhaustedPatchNumber( )
    {
        var error = Assert.Throws<ToolException>( ( ) => ReleaseVersion.Parse( $"1.2.{int.MaxValue}" ).NextPatch( ) );

        Assert.Equal( ExitCodes.InvalidArguments, error.ExitCode );
    }

    [Fact]
    public void ProcessArgumentsAreQuotedForDiagnostics( )
    {
        var value = ProcessRunner.FormatCommand( "git", ["status", "two words", "safe/path"] );
        Assert.Equal( "git status 'two words' safe/path", value );
    }

    [Fact]
    public void RustMaskPreservesLineNumbersAndRemovesCommentsAndStrings( )
    {
        const string source = "fn one() { /* { */ call(); }\n// call()\nlet text = \"call()\";\n";
        var masked = RustSource.StripRustCommentsAndStrings( source );
        Assert.Equal( source.Count( character => character == '\n' ), masked.Count( character => character == '\n' ) );
        Assert.Single( System.Text.RegularExpressions.Regex.Matches( masked, @"\bcall\s*\(" ).Cast<System.Text.RegularExpressions.Match>( ) );
    }

    [Fact]
    public void CfgTestRemovalHandlesAllTestPredicate( )
    {
        const string source = "live();\n#[cfg(all(test, feature = \"x\"))]\nfn test_only() { hidden(); }\nlive_again();";
        var production = RustSource.RemoveCfgTestBlocks( RustSource.StripRustCommentsAndStrings( source ) );
        Assert.Contains( "live();", production );
        Assert.Contains( "live_again();", production );
        Assert.DoesNotContain( "hidden", production );
    }

    // The report never fails: discovery and read problems show in its status lines.
    [Fact]
    public async Task CodeHealthReportsDiscoveryWarningsAndUnreadableFiles( )
    {
        if ( !OperatingSystem.IsLinux( ) || Environment.IsPrivilegedProcess )
        {
            return;
        }

        using var directory = new TemporaryDirectory( "wayscriber-code-health-test" );
        Directory.CreateDirectory( Path.Combine( directory.Path, "src" ) );
        File.WriteAllText( Path.Combine( directory.Path, "src/lib.rs" ), "fn main() {}\n" );
        var locked = Path.Combine( directory.Path, "src/locked.rs" );
        File.WriteAllText( locked, "fn hidden() {}\n" );
        File.SetUnixFileMode( locked, UnixFileMode.None );
        var git = new GitListing( ExitCodes.Success, "src/lib.rs\nsrc/locked.rs\n", "warning: index is stale\n" );

        var report = await RunCodeHealth( directory.Path, git );

        Assert.Contains( "status=error\nerrors=read\nwarnings=discovery\n" +
            "warning=git_ls_files_stderr\nwarning_detail=warning: index is stale\n", report, StringComparison.Ordinal );
        Assert.Contains( "\nrust_files=2\n", report, StringComparison.Ordinal );
        Assert.Contains( "\nread_errors=1\n", report, StringComparison.Ordinal );
        Assert.Contains( "\nread_errors:\n  src/locked.rs\t", report, StringComparison.Ordinal );
    }

    [Fact]
    public async Task CodeHealthReportsAFailedDiscovery( )
    {
        using var directory = new TemporaryDirectory( "wayscriber-code-health-test" );
        var git = new GitListing( GitFatalExitCode, string.Empty, "fatal: not a git repository\n" );

        var report = await RunCodeHealth( directory.Path, git );

        Assert.StartsWith( "report=wayscriber-code-health\nstatus=error\nerrors=discovery\nerror=git_ls_files_failed\n" +
            "error_detail=fatal: not a git repository\n", report, StringComparison.Ordinal );
        Assert.Contains( "\nrust_files=0\n", report, StringComparison.Ordinal );
    }

    [Fact]
    public void InstallerManifestRejectsDuplicatePaths( )
    {
        const string source = "# ARCH_INSTALL_MANIFEST_BEGIN\nrelease_manifest() {\nprintf '%s\\n' \\\n'0755 bin/wayscriber' \\\n'0644 bin/wayscriber'\n}\n# ARCH_INSTALL_MANIFEST_END\n";
        Assert.Throws<ToolException>( ( ) => PackagingCommands.ParseInstallerManifest( source ) );
    }

    [Fact]
    public void AtomicFileSetReplacesCompleteContent( )
    {
        using var directory = new TemporaryDirectory( "wayscriber-test" );
        var path = Path.Combine( directory.Path, "value" );
        File.WriteAllText( path, "before" );
        var files = new AtomicFileSet( );
        files.Add( path, "after" );
        files.Commit( );
        Assert.Equal( "after", File.ReadAllText( path ) );
    }

    [Fact]
    public void CurrentAssetManifestsProduceAllRecipeChannels( )
    {
        var root = TestRepository.Root;
        var recipe = AssetsCommand.CreateRecipe( root );
        Assert.NotNull( recipe["source"] );
        Assert.NotNull( recipe["bin"] );
        Assert.NotNull( recipe["configurator"] );
    }

    [Fact]
    public void CurrentVersionMetadataIsConsistent( )
    {
        Assert.Empty( VersionCommands.Validate( TestRepository.Root ) );
    }

    private static async Task<string> RunCodeHealth( string root, IProcessRunner git )
    {
        using var output = new StringWriter( );
        var command = ReportCommands.Commands.Single( command => command.Name == CommandNames.CodeHealth );
        var context = new ToolContext( root, output, TextWriter.Null, git, CancellationToken.None );

        Assert.Equal( ExitCodes.Success, await command.Handler( context, [] ) );

        return output.ToString( );
    }

    // Answers the report's `git ls-files` with a fixed listing, rejecting an exit
    // code the request does not allow as the real process runner does.
    private sealed class GitListing( int exitCode, string output, string error ) : IProcessRunner
    {
        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            Assert.Equal( Programs.Git, request.FileName );

            var result = new ProcessResult( exitCode, output, error );
            if ( !result.IsSuccess && request.AllowedExitCodes?.Contains( exitCode ) != true )
            {
                throw new ToolException( $"{request.FileName} exited with code {exitCode}.", exitCode );
            }

            return Task.FromResult( result );
        }
    }
}
