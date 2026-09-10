using System.Text;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class CoreTests
{
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

    [Fact]
    public void ReleaseVersionRejectsMalformedInput( ) =>
        Assert.Throws<ToolException>( ( ) => ReleaseVersion.Parse( "1.2" ) );

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
        var masked = ConfigWriterAudit.StripRustCommentsAndStrings( source );
        Assert.Equal( source.Count( character => character == '\n' ), masked.Count( character => character == '\n' ) );
        Assert.Single( System.Text.RegularExpressions.Regex.Matches( masked, @"\bcall\s*\(" ).Cast<System.Text.RegularExpressions.Match>( ) );
    }

    [Fact]
    public void CfgTestRemovalHandlesAllTestPredicate( )
    {
        const string source = "live();\n#[cfg(all(test, feature = \"x\"))]\nfn test_only() { hidden(); }\nlive_again();";
        var production = ConfigWriterAudit.RemoveCfgTestBlocks( ConfigWriterAudit.StripRustCommentsAndStrings( source ) );
        Assert.Contains( "live();", production );
        Assert.Contains( "live_again();", production );
        Assert.DoesNotContain( "hidden", production );
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
        var root = FindRepository( );
        var recipe = AssetsCommand.CreateRecipe( root );
        Assert.NotNull( recipe["source"] );
        Assert.NotNull( recipe["bin"] );
        Assert.NotNull( recipe["configurator"] );
    }

    [Fact]
    public void CurrentVersionMetadataIsConsistent( )
    {
        Assert.Empty( VersionCommands.Validate( FindRepository( ) ) );
    }

    private static string FindRepository( )
    {
        var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
        return Path.GetFullPath( Path.Combine( directory, ".." ) );
    }
}
