using System.Text;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

// Partial source guard: no alias resolution, macro expansion, or dependency graph.
internal static class SharedDependencyGuard
{
    private static readonly Regex NonCode = new(
        "r(?<hashes>#{0,16})\".*?\"\\k<hashes>|\"(?:\\\\.|[^\"\\\\])*\"|'(?:\\\\.|[^'\\\\\\n])'|//[^\\n]*|/\\*",
        RegexOptions.Singleline );
    private static readonly Regex Tokens = new( @"r#[A-Za-z_][A-Za-z_0-9]*|[A-Za-z_][A-Za-z_0-9]*|::|[{},;*]" );
    private static readonly Regex CommentMarkers = new( @"/\*|\*/" );

    public static bool HasUpwardPath( string source, string relativePath, IReadOnlySet<string> forbidden )
    {
        var tokens = Tokens.Matches( StripNonCode( source ) )
            .Select( match => match.Value.StartsWith( "r#", StringComparison.Ordinal ) ? match.Value[2..] : match.Value ).ToArray( );
        var module = relativePath[..^3].Split( '/' ).Skip( 1 ).ToList( );
        if ( module[^1] == "mod" )
        {
            module.RemoveAt( module.Count - 1 );
        }

        for ( var index = 0; index + 1 < tokens.Length; index++ )
        {
            if ( tokens[index] is "crate" or "super" or "self" && tokens[index + 1] == "::" &&
                Traverse( tokens, index, module, forbidden ).Rejected )
            {
                return true;
            }
        }
        return false;
    }

    private static (bool Rejected, int Index) Traverse(
        string[] tokens, int index, List<string> prefix, IReadOnlySet<string> forbidden )
    {
        var path = new List<string>( prefix );
        while ( index < tokens.Length )
        {
            var token = tokens[index];
            if ( token is "," or ";" or "}" or "as" )
            {
                break;
            }
            if ( token == "{" )
            {
                return TraverseGroup( tokens, index + 1, path, forbidden );
            }
            if ( token == "crate" )
            {
                path.Clear( );
            }
            else if ( token == "super" && path.Count > 0 )
            {
                path.RemoveAt( path.Count - 1 );
            }
            else if ( token is not ("super" or "self" or "::" or "*") )
            {
                path.Add( token );
            }
            if ( path.Count > 0 && forbidden.Contains( path[0] ) )
            {
                return (true, index);
            }
            index++;
            if ( index >= tokens.Length || tokens[index] != "::" )
            {
                break;
            }
            index++;
        }
        return (false, index);
    }

    private static (bool Rejected, int Index) TraverseGroup(
        string[] tokens, int index, List<string> prefix, IReadOnlySet<string> forbidden )
    {
        while ( index < tokens.Length && tokens[index] != "}" )
        {
            var result = Traverse( tokens, index, prefix, forbidden );
            if ( result.Rejected )
            {
                return result;
            }
            index = result.Index;
            if ( index < tokens.Length && tokens[index] == "as" )
            {
                index += 2;
            }
            if ( index < tokens.Length && tokens[index] == "," )
            {
                index++;
            }
            else if ( index < tokens.Length && tokens[index] != "}" )
            {
                break;
            }
        }
        return (false, index + 1);
    }

    private static string StripNonCode( string source )
    {
        var pieces = new StringBuilder( );
        var position = 0;
        var match = NonCode.Match( source, position );
        while ( match.Success )
        {
            pieces.Append( source, position, match.Index - position );
            position = match.Index + match.Length;
            if ( match.Value == "/*" )
            {
                position = SkipBlockComment( source, position );
            }
            pieces.Append( ' ' );
            match = NonCode.Match( source, position );
        }
        pieces.Append( source, position, source.Length - position );
        return pieces.ToString( );
    }

    private static int SkipBlockComment( string source, int position )
    {
        var depth = 1;
        var marker = CommentMarkers.Match( source, position );
        while ( depth > 0 && marker.Success )
        {
            depth += marker.Value == "/*" ? 1 : -1;
            position = marker.Index + marker.Length;
            marker = CommentMarkers.Match( source, position );
        }
        return depth > 0 ? source.Length : position;
    }
}
