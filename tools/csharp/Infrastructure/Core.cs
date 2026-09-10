using System.Globalization;

namespace Wayscriber.Tools;

internal enum SideEffect
{
    ReadOnly,
    FixtureMutating,
    MachineMutating,
    RemoteMutating,
    ForegroundSensitive,
}

internal sealed record ToolCommand(
    string Area,
    string Name,
    string Description,
    SideEffect SideEffect,
    Func<ToolContext, string[], Task<int>> Handler );

internal sealed record ToolContext(
    string RepositoryRoot,
    TextWriter Output,
    TextWriter Error,
    IProcessRunner Processes,
    CancellationToken CancellationToken,
    Func<string, string?>? EnvironmentReader = null )
{
    public string Path( params string[] parts ) =>
        System.IO.Path.GetFullPath( System.IO.Path.Combine( [RepositoryRoot, .. parts] ) );

    public string? Environment( string name )
    {
        var value = EnvironmentReader is null
            ? System.Environment.GetEnvironmentVariable( name )
            : EnvironmentReader( name );

        return string.IsNullOrWhiteSpace( value ) ? null : value;
    }
}

internal sealed class ToolException( string message, int exitCode = ExitCodes.Failure ) : Exception( message )
{
    public int ExitCode { get; } = exitCode;
}

internal static class RepositoryLocator
{
    public static string Locate( )
    {
        var entryDirectory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string;
        if ( string.IsNullOrWhiteSpace( entryDirectory ) )
        {
            var entryPath = AppContext.GetData( "EntryPointFilePath" ) as string;
            entryDirectory = entryPath is null ? null : System.IO.Path.GetDirectoryName( entryPath );
        }

        if ( entryDirectory is null )
        {
            throw new ToolException( "Cannot determine the C# entry-point directory." );
        }

        var root = System.IO.Path.GetFullPath( System.IO.Path.Combine( entryDirectory, ".." ) );
        if ( !File.Exists( System.IO.Path.Combine( root, RepositoryPaths.CargoManifest ) ) ||
            !File.Exists( System.IO.Path.Combine( root, RepositoryPaths.ToolsDirectory, RepositoryNames.ToolEntryFile ) ) )
        {
            throw new ToolException( $"The entry point is not inside a Wayscriber repository: {entryDirectory}" );
        }

        return root;
    }
}

internal sealed class Arguments( IEnumerable<string> values )
{
    private const int OptionTokenCount = 2;
    private readonly List<string> _values = [.. values];

    public IReadOnlyList<string> Remaining => _values;

    public bool TakeFlag( string name ) => _values.Remove( name );

    public string? TakeOption( string name, bool required = false )
    {
        var index = _values.IndexOf( name );
        if ( index < 0 )
        {
            if ( required )
            {
                throw new ToolException( $"{name} requires a value.", ExitCodes.InvalidArguments );
            }
            return null;
        }
        if ( index + 1 >= _values.Count )
        {
            throw new ToolException( $"{name} requires a value.", ExitCodes.InvalidArguments );
        }

        var result = _values[index + 1];
        _values.RemoveRange( index, OptionTokenCount );
        return result;
    }

    public string SinglePositional( string usage )
    {
        if ( _values.Count != 1 )
        {
            throw new ToolException( usage, ExitCodes.InvalidArguments );
        }
        return _values[0];
    }

    public void RequireEmpty( string usage )
    {
        if ( _values.Count != 0 )
        {
            throw new ToolException( $"{usage}\nUnexpected argument: {_values[0]}", ExitCodes.InvalidArguments );
        }
    }

    public int TakeIntOption( string name, int fallback )
    {
        var text = TakeOption( name );
        if ( text is null )
        {
            return fallback;
        }
        if ( !int.TryParse( text, NumberStyles.None, CultureInfo.InvariantCulture, out var value ) )
        {
            throw new ToolException( $"{name} must be an integer.", ExitCodes.InvalidArguments );
        }
        return value;
    }
}
