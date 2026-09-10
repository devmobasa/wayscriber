namespace Wayscriber.Tools;

internal static class ToolApplication
{
    private const int HelpAreaWidth = 10;
    private const int HelpCommandWidth = 26;

    private static readonly IReadOnlyList<ToolCommand> Commands =
    [
        .. AssetsCommand.Commands,
        .. DevelopmentCommands.Commands,
        .. ChecksCommand.Commands,
        .. VersionCommands.Commands,
        .. NativeDesktopCommands.Commands,
        .. PackagingCommands.Commands,
        .. ReleaseAurCommands.Commands,
        .. ReportCommands.Commands,
    ];

    public static async Task<int> RunAsync( string[] args )
    {
        using var cancellation = new CancellationTokenSource( );
        Console.CancelKeyPress += ( _, eventArgs ) =>
        {
            eventArgs.Cancel = true;
            cancellation.Cancel( );
        };

        try
        {
            if ( args.Length == 0 || args is [CommandLineOptions.ShortHelp or CommandLineOptions.Help] )
            {
                PrintHelp( Console.Out );
                return ExitCodes.Success;
            }
            if ( args is [CommandNames.Help, ..] )
            {
                PrintHelp( Console.Out, args.Length > 1 ? args[1] : null );
                return ExitCodes.Success;
            }
            if ( args.Length < 2 )
            {
                throw new ToolException( "Expected AREA COMMAND. Run with --help for available commands.", ExitCodes.InvalidArguments );
            }

            var command = Commands.SingleOrDefault( candidate =>
                candidate.Area == args[0] && candidate.Name == args[1] );
            if ( command is null )
            {
                throw new ToolException( $"Unknown command: {args[0]} {args[1]}", ExitCodes.InvalidArguments );
            }

            var root = RepositoryLocator.Locate( );
            var context = new ToolContext( root, Console.Out, Console.Error, new ProcessRunner( Console.Out, Console.Error ), cancellation.Token );
            return await command.Handler( context, args[2..] );
        }
        catch ( OperationCanceledException )
        {
            await Console.Error.WriteLineAsync( ToolMessages.Canceled );
            return ExitCodes.Canceled;
        }
        catch ( ToolException error )
        {
            await Console.Error.WriteLineAsync( error.Message );
            return error.ExitCode;
        }
        catch ( Exception error )
        {
            await Console.Error.WriteLineAsync( $"{ToolMessages.UnexpectedFailurePrefix}{error.Message}" );
            return ExitCodes.SoftwareError;
        }
    }

    internal static Task<int> RunNestedAsync( ToolContext context, string area, string name, string[] args )
    {
        var command = Commands.SingleOrDefault( candidate => candidate.Area == area && candidate.Name == name );
        if ( command is null )
        {
            throw new ToolException( $"Unknown command: {area} {name}", ExitCodes.InvalidArguments );
        }

        return command.Handler( context, args );
    }

    private static void PrintHelp( TextWriter output, string? area = null )
    {
        output.WriteLine( "Wayscriber repository automation" );
        output.WriteLine( "Usage: dotnet run tools/wayscriber.cs --no-build -- AREA COMMAND [OPTIONS]" );
        output.WriteLine( );
        foreach ( var command in Commands.Where( command => area is null || command.Area == area )
                     .OrderBy( command => command.Area ).ThenBy( command => command.Name ) )
        {
            output.WriteLine( $"  {command.Area,-HelpAreaWidth} {command.Name,-HelpCommandWidth} {command.Description} [{command.SideEffect}]" );
        }
    }
}
