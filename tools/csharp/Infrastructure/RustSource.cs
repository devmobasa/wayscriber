using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

// Masks Rust source for line-oriented reports: comments and literals become
// spaces, and `#[cfg(test)]` items can be removed, keeping every offset.
internal static class RustSource
{
    internal static string RemoveCfgTestBlocks( string text )
    {
        var chars = text.ToCharArray( );
        foreach ( Match marker in Regex.Matches( text, @"#\[cfg\((?!\s*not\s*\(\s*test\s*\))(?=[^]]*\btest\b)[^]]*\)\]" ) )
        {
            var opening = text.IndexOfAny( ['{', ';'], marker.Index + marker.Length );
            if ( opening < 0 )
            {
                continue;
            }
            var end = opening + 1;
            if ( text[opening] == '{' )
            {
                var depth = 1;
                while ( end < text.Length && depth > 0 )
                {
                    if ( text[end] == '{' )
                    {
                        depth++;
                    }
                    else if ( text[end] == '}' )
                    {
                        depth--;
                    }

                    end++;
                }
            }
            for ( var index = marker.Index; index < end; index++ )
            {
                if ( chars[index] != '\n' )
                {
                    chars[index] = ' ';
                }
            }
        }
        return new string( chars );
    }

    internal static string StripRustCommentsAndStrings( string text )
    {
        var output = text.ToCharArray( );
        var index = 0;
        var blockDepth = 0;
        while ( index < text.Length )
        {
            if ( blockDepth == 0 && StartsWith( text, index, '/', '/' ) )
            {
                MaskLineComment( text, output, ref index );
            }
            else if ( StartsWith( text, index, '/', '*' ) )
            {
                MaskPair( output, ref index );
                blockDepth++;
            }
            else if ( blockDepth > 0 )
            {
                MaskBlockCommentCharacter( text, output, ref index, ref blockDepth );
            }
            else if ( text[index] == '"' )
            {
                MaskString( text, output, ref index );
            }
            else
            {
                index++;
            }
        }
        return new string( output );
    }

    private static bool StartsWith( string text, int index, char first, char second ) =>
        index + 1 < text.Length && text[index] == first && text[index + 1] == second;

    private static void MaskLineComment( string text, char[] output, ref int index )
    {
        while ( index < text.Length && text[index] != '\n' )
        {
            output[index++] = ' ';
        }
    }

    private static void MaskBlockCommentCharacter( string text, char[] output, ref int index, ref int blockDepth )
    {
        if ( StartsWith( text, index, '*', '/' ) )
        {
            MaskPair( output, ref index );
            blockDepth--;
            return;
        }

        if ( text[index] != '\n' )
        {
            output[index] = ' ';
        }
        index++;
    }

    private static void MaskPair( char[] output, ref int index )
    {
        output[index++] = ' ';
        output[index++] = ' ';
    }

    private static void MaskString( string text, char[] output, ref int index )
    {
        output[index++] = ' ';
        while ( index < text.Length )
        {
            var character = text[index];
            if ( character != '\n' )
            {
                output[index] = ' ';
            }
            index++;

            if ( character == '\\' && index < text.Length )
            {
                if ( text[index] != '\n' )
                {
                    output[index] = ' ';
                }
                index++;
                continue;
            }

            if ( character == '"' )
            {
                return;
            }
        }
    }
}
