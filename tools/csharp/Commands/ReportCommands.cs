using System.Text;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal static class ReportCommands
{
    private const int MaximumFunctionLines = 120;
    private const int LargeFileLineCount = 500;
    private const int OffsetNotFound = -1;
    private const int ExitCodeLimit = 256;

    // `git ls-files` reports its own failures; the report records them instead of stopping.
    private static readonly IReadOnlySet<int> AnyExitCode = Enumerable.Range( 0, ExitCodeLimit ).ToHashSet( );

    public static IReadOnlyList<ToolCommand> Commands
    {
        get;
    } =
    [
        new( CommandAreas.Report, CommandNames.CodeHealth, "Report Rust maintainability metrics.", SideEffect.ReadOnly, CodeHealth ),
    ];

    private static async Task<int> CodeHealth( ToolContext context, string[] args )
    {
        var parsed = new Arguments( args );
        var outputPath = parsed.TakeOption( "--output" );
        var githubSummary = parsed.TakeFlag( "--github-summary" );

        parsed.RequireEmpty( "report code-health [--output FILE] [--github-summary]" );

        var discovery = await DiscoverRustFiles( context );
        var paths = discovery.Paths;

        var readErrors = new List<string>( );
        var files = new List<(int Lines, string Path)>( );
        var functions = new List<(int Lines, string Path, int Line, string Name)>( );
        var directWrites = new List<string>( );
        var counts = new Dictionary<string, int> { ["unwrap"] = 0, ["expect"] = 0, ["panic"] = 0, ["unsafe"] = 0 };
        var allowDead = 0;
        var allowUnused = 0;
        var total = 0;

        foreach ( var relative in paths )
        {
            string text;
            try
            {
                text = Files.Read( context.Path( relative.Split( '/' ) ) );
            }
            catch ( Exception error ) when ( error is IOException or UnauthorizedAccessException )
            {
                readErrors.Add( $"{relative}\t{error.Message}" );
                continue;
            }

            var lines = text.Length == 0 ? 0 : text.Count( character => character == '\n' ) + (text.EndsWith( '\n' ) ? 0 : 1);
            total += lines;
            files.Add( (lines, relative) );
            var code = RustSource.StripRustCommentsAndStrings( text );
            foreach ( Match match in Regex.Matches( code, @"\bfn\s+([A-Za-z_][A-Za-z0-9_]*)\s*(?:<[^>{;]*>)?\s*\(" ) )
            {
                var opening = FindBody( code, match.Index + match.Length );
                if ( opening < 0 )
                {
                    continue;
                }

                var end = FindBlockEnd( code, opening );
                if ( end < 0 )
                {
                    continue;
                }

                var startLine = LineAt( code, match.Index );
                var endLine = LineAt( code, end );
                if ( endLine - startLine + 1 > MaximumFunctionLines )
                {
                    functions.Add( (endLine - startLine + 1, relative, startLine, match.Groups[1].Value) );
                }
            }
            allowDead += Regex.Matches( text, @"#\s*\[\s*allow\s*\([^)]*\bdead_code\b[^)]*\)\s*\]" ).Count;
            allowUnused += Regex.Matches( text, @"#\s*\[\s*allow\s*\([^)]*\bunused_imports\b[^)]*\)\s*\]" ).Count;
            if ( IsTestPath( relative ) )
            {
                continue;
            }

            var production = RustSource.RemoveCfgTestBlocks( code );

            counts["unwrap"] += Regex.Matches( production, @"\.\s*unwrap\s*\(" ).Count;
            counts["expect"] += Regex.Matches( production, @"\.\s*expect\s*\(" ).Count;
            counts["panic"] += Regex.Matches( production, @"\bpanic\s*!" ).Count;
            counts["unsafe"] += Regex.Matches( production, @"\bunsafe\b" ).Count;

            if ( Regex.IsMatch( production, @"(?<![\w:])(?:std::fs::write|fs::write)\s*\(" ) )
            {
                directWrites.Add( relative );
            }
        }

        files.Sort( ( left, right ) =>
        {
            if ( right.Lines != left.Lines )
            {
                return right.Lines.CompareTo( left.Lines );
            }
            else
            {
                return string.CompareOrdinal( right.Path, left.Path );
            }
        } );

        functions.Sort( ( left, right ) =>
        {
            var comparison = right.Lines.CompareTo( left.Lines );
            if ( comparison != 0 )
            {
                return comparison;
            }
            comparison = right.Line.CompareTo( left.Line );
            if ( comparison != 0 )
            {
                return comparison;
            }
            comparison = string.CompareOrdinal( right.Path, left.Path );
            return comparison != 0 ? comparison : string.CompareOrdinal( right.Name, left.Name );
        } );

        directWrites.Sort( StringComparer.Ordinal );
        var report = new StringBuilder( );

        report.AppendLine( "report=wayscriber-code-health" );
        AppendStatus( report, discovery, readErrors.Count > 0 );
        report.AppendLine( $"repo_root={context.RepositoryRoot}" )
            .AppendLine( $"rust_files={paths.Length}" ).AppendLine( $"rust_physical_lines={total}" )
            .AppendLine( $"files_over_{LargeFileLineCount}={files.Count( item => item.Lines > LargeFileLineCount )}" )
            .AppendLine( $"functions_over_120={functions.Count}" ).AppendLine( $"production_unwrap={counts["unwrap"]}" ).AppendLine( $"production_expect={counts["expect"]}" )
            .AppendLine( $"production_panic={counts["panic"]}" ).AppendLine( $"production_unsafe={counts["unsafe"]}" ).AppendLine( $"allow_dead_code={allowDead}" )
            .AppendLine( $"allow_unused_imports={allowUnused}" ).AppendLine( $"direct_fs_write_files={directWrites.Count}" )
            .AppendLine( $"read_errors={readErrors.Count}" );

        var filesOverLimit = files.Where( item => item.Lines > LargeFileLineCount ).Select( item => $"{item.Lines}\t{item.Path}" );

        Section( report, $"files_over_{LargeFileLineCount}", filesOverLimit );
        Section( report, "functions_over_120", functions.Select( item => $"{item.Lines}\t{item.Path}:{item.Line}\t{item.Name}" ) );
        Section( report, "direct_fs_write_files", directWrites );
        Section( report, "read_errors", readErrors );

        if ( outputPath is not null )
        {
            Files.WriteAtomic( Path.GetFullPath( outputPath, context.RepositoryRoot ), report.ToString( ) );
        }
        else
        {
            await context.Output.WriteAsync( report.ToString( ) );
        }

        if ( githubSummary )
        {
            var headline = string.Join( '\n', report.ToString( ).Split( '\n' ).TakeWhile( line => line.Length > 0 ) );
            if ( context.Environment( EnvironmentVariables.GitHubStepSummary ) is { Length: > 0 } summaryPath )
            {
                await File.AppendAllTextAsync( summaryPath, $"### Wayscriber code health\n\n```text\n{headline}\n```\n", context.CancellationToken );
            }
            else
            {
                await context.Output.WriteLineAsync( headline );
            }
        }
        return ExitCodes.Success;
    }

    // The report is observational: a discovery or read problem is reported in its
    // status lines, and the report still describes whatever it could read.
    private static async Task<RustFileDiscovery> DiscoverRustFiles( ToolContext context )
    {
        ProcessResult git;
        try
        {
            git = await context.Run( Programs.Git, ["ls-files", "-co", "--exclude-standard", CommandLineOptions.EndOfOptions, "*.rs"],
                capture: true, trace: false, allowedExitCodes: AnyExitCode );
        }
        catch ( ToolException error ) when ( error.ExitCode == ExitCodes.CommandNotFound )
        {
            return new( [], new( "git_unavailable", error.Message ), null );
        }

        var stderr = git.StandardError.Trim( );
        if ( !git.IsSuccess )
        {
            return new( [], new( "git_ls_files_failed", stderr ), null );
        }

        var paths = git.StandardOutput.Split( '\n', StringSplitOptions.RemoveEmptyEntries ).Distinct( )
            .Where( relative => File.Exists( context.Path( relative.Split( '/' ) ) ) )
            .ToArray( );
        return new( paths, null, stderr.Length > 0 ? new ReportProblem( "git_ls_files_stderr", stderr ) : null );
    }

    private static void AppendStatus( StringBuilder report, RustFileDiscovery discovery, bool readFailed )
    {
        var errors = new List<string>( );
        var warnings = new List<string>( );
        if ( discovery.Error is not null )
        {
            errors.Add( "discovery" );
        }
        if ( readFailed )
        {
            errors.Add( "read" );
        }
        if ( discovery.Warning is not null )
        {
            warnings.Add( "discovery" );
        }

        var status = errors.Count > 0 ? "error" : warnings.Count > 0 ? "warning" : "ok";
        report.AppendLine( $"status={status}" );
        if ( errors.Count > 0 )
        {
            report.AppendLine( $"errors={string.Join( ',', errors )}" );
        }
        if ( warnings.Count > 0 )
        {
            report.AppendLine( $"warnings={string.Join( ',', warnings )}" );
        }
        AppendProblem( report, "error", discovery.Error );
        AppendProblem( report, "warning", discovery.Warning );
    }

    private static void AppendProblem( StringBuilder report, string kind, ReportProblem? problem )
    {
        if ( problem is not { } found )
        {
            return;
        }

        report.AppendLine( $"{kind}={found.Name}" );
        if ( found.Detail.Length > 0 )
        {
            report.AppendLine( $"{kind}_detail={found.Detail}" );
        }
    }

    private sealed record RustFileDiscovery( string[] Paths, ReportProblem? Error, ReportProblem? Warning );

    // A named problem and its detail, printed as `error=`/`warning=` lines.
    private sealed record ReportProblem( string Name, string Detail );

    private static void Section( StringBuilder report, string name, IEnumerable<string> rows )
    {
        report.AppendLine( ).AppendLine( name + ":" );
        var any = false;
        foreach ( var row in rows )
        {
            report.AppendLine( "  " + row );
            any = true;
        }
        if ( !any )
        {
            report.AppendLine( "  none" );
        }
    }

    private static bool IsTestPath( string path )
    {
        var parts = path.Split( '/' );
        var name = Path.GetFileName( path );
        return parts.Contains( "tests" ) || name is "tests.rs" or "test_helpers.rs" or "test_support.rs" || name.StartsWith( "test_" ) || name.EndsWith( "_tests.rs" );
    }
    private static int LineAt( string text, int offset ) => text[..Math.Min( offset, text.Length )].Count( character => character == '\n' ) + 1;
    private static int FindBody( string code, int start )
    {
        var parens = 1;
        var brackets = 0;
        for ( var index = start; index < code.Length; index++ )
        {
            switch ( code[index] )
            {
                case '(':
                    parens++;
                    break;
                case ')':
                    parens--;
                    break;
                case '[':
                    brackets++;
                    break;
                case ']':
                    brackets--;
                    break;
                case '{' when parens == 0 && brackets == 0:
                    return index;
                case ';' when parens == 0 && brackets == 0:
                    return OffsetNotFound;
            }
        }
        return OffsetNotFound;
    }
    private static int FindBlockEnd( string code, int opening )
    {
        var depth = 0;
        for ( var index = opening; index < code.Length; index++ )
        {
            if ( code[index] == '{' )
            {
                depth++;
            }
            else if ( code[index] == '}' && --depth == 0 )
            {
                return index;
            }
        }
        return OffsetNotFound;
    }
}
