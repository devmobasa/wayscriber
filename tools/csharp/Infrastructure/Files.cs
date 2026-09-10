using System.Security.Cryptography;
using System.Text;
using System.Text.RegularExpressions;

namespace Wayscriber.Tools;

internal sealed class TemporaryDirectory : IDisposable
{
    public TemporaryDirectory( string prefix = "wayscriber-tools" )
    {
        Path = System.IO.Path.Combine( System.IO.Path.GetTempPath( ), $"{prefix}-{Guid.NewGuid( ):N}" );
        Directory.CreateDirectory( Path );
        if ( !OperatingSystem.IsWindows( ) )
        {
            File.SetUnixFileMode( Path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
        }
    }

    public string Path
    {
        get;
    }

    public void Dispose( )
    {
        try
        {
            Directory.Delete( Path, recursive: true );
        }
        catch ( IOException )
        {
        }
    }
}

internal static partial class Files
{
    private static readonly TimeSpan RegexTimeout = TimeSpan.FromSeconds( 2 );

    public static string Read( string path ) => File.ReadAllText( path, Encoding.UTF8 );

    public static void WriteAtomic( string path, string content, UnixFileMode? mode = null )
    {
        var directory = System.IO.Path.GetDirectoryName( path );
        if ( directory is null )
        {
            throw new ToolException( $"No parent directory for {path}." );
        }

        Directory.CreateDirectory( directory );
        var temporary = System.IO.Path.Combine( directory, $".{System.IO.Path.GetFileName( path )}.{Guid.NewGuid( ):N}.tmp" );
        try
        {
            File.WriteAllText( temporary, content, new UTF8Encoding( encoderShouldEmitUTF8Identifier: false ) );
            if ( !OperatingSystem.IsWindows( ) && mode is not null )
            {
                File.SetUnixFileMode( temporary, mode.Value );
            }
            else if ( File.Exists( path ) && !OperatingSystem.IsWindows( ) )
            {
                File.SetUnixFileMode( temporary, File.GetUnixFileMode( path ) );
            }
            File.Move( temporary, path, overwrite: true );
        }
        finally
        {
            File.Delete( temporary );
        }
    }

    public static void WriteAtomic( string path, ReadOnlySpan<byte> content, UnixFileMode? mode = null )
    {
        var directory = System.IO.Path.GetDirectoryName( path );
        if ( directory is null )
        {
            throw new ToolException( $"No parent directory for {path}." );
        }

        Directory.CreateDirectory( directory );
        var temporary = System.IO.Path.Combine( directory, $".{System.IO.Path.GetFileName( path )}.{Guid.NewGuid( ):N}.tmp" );
        try
        {
            File.WriteAllBytes( temporary, content.ToArray( ) );
            if ( !OperatingSystem.IsWindows( ) && mode is not null )
            {
                File.SetUnixFileMode( temporary, mode.Value );
            }
            else if ( File.Exists( path ) && !OperatingSystem.IsWindows( ) )
            {
                File.SetUnixFileMode( temporary, File.GetUnixFileMode( path ) );
            }
            File.Move( temporary, path, overwrite: true );
        }
        finally { File.Delete( temporary ); }
    }

    public static string Sha256( string path )
    {
        using var stream = File.OpenRead( path );
        return Convert.ToHexStringLower( SHA256.HashData( stream ) );
    }

    public static string RequireSingleMatch( string text, string pattern, string label, RegexOptions options = RegexOptions.Multiline )
    {
        var matches = Regex.Matches( text, pattern, options );
        if ( matches.Count != 1 )
        {
            throw new ToolException( $"{label}: expected one match, found {matches.Count}." );
        }
        return matches[0].Groups.Count > 1 ? matches[0].Groups[1].Value : matches[0].Value;
    }

    public static string ReplaceSingle( string text, string pattern, string replacement, string label, RegexOptions options = RegexOptions.Multiline )
    {
        var matches = Regex.Matches( text, pattern, options );
        if ( matches.Count != 1 )
        {
            throw new ToolException( $"{label}: expected one replacement target, found {matches.Count}." );
        }
        return Regex.Replace( text, pattern, replacement, options, RegexTimeout );
    }
}

internal sealed class AtomicFileSet
{
    private readonly Dictionary<string, string> _outputs = new( StringComparer.Ordinal );

    public void Add( string path, string content ) => _outputs.Add( System.IO.Path.GetFullPath( path ), content );

    public void Commit( )
    {
        var originals = _outputs.Keys.ToDictionary(
            path => path,
            path => File.Exists( path ) ? File.ReadAllBytes( path ) : null,
            StringComparer.Ordinal );
        var committed = new List<string>( );
        try
        {
            foreach ( var pair in _outputs )
            {
                Files.WriteAtomic( pair.Key, pair.Value );
                committed.Add( pair.Key );
            }
        }
        catch
        {
            foreach ( var path in committed.AsEnumerable( ).Reverse( ) )
            {
                if ( originals[path] is { } bytes )
                {
                    File.WriteAllBytes( path, bytes );
                }
                else
                {
                    File.Delete( path );
                }
            }
            throw;
        }
    }
}
