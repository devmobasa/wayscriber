namespace Wayscriber.Tools;

internal static partial class NativeDesktopCommands
{
    internal enum WayscriberServiceState
    {
        Unavailable,
        Inactive,
        Active,
    }

    internal static async Task<WayscriberServiceState> InspectWayscriberService( ToolContext context )
    {
        if ( !await CommandExists( context, Programs.SystemControl ) )
        {
            return WayscriberServiceState.Unavailable;
        }

        var result = await context.Run( Programs.SystemControl,
            ["--user", "show", RepositoryNames.UserServiceFile, "-p", "ActiveState", "--value"],
            environment: new Dictionary<string, string?> { [EnvironmentVariables.LocaleAll] = "C" },
            capture: true, allowedExitCodes: new HashSet<int> { ExitCodes.Failure } );
        if ( !result.IsSuccess )
        {
            if ( result.StandardError.Contains( "Failed to connect to bus", StringComparison.Ordinal ) ||
                 result.StandardError.Contains( "Failed to connect to user scope bus", StringComparison.Ordinal ) )
            {
                return WayscriberServiceState.Unavailable;
            }

            throw new ToolException( $"Could not inspect wayscriber.service before selecting a new app/broker cohort: {result.StandardError.Trim( )}" );
        }

        return result.StandardOutput.Trim( ) switch
        {
            "active" => WayscriberServiceState.Active,
            "inactive" or "failed" => WayscriberServiceState.Inactive,
            var state => throw new ToolException( $"wayscriber.service is in state '{state}'; retry after it settles." )
        };
    }

    private static async Task<bool> ServiceUsesDestination( ToolContext context, string destination )
    {
        var result = await context.Run( Programs.SystemControl,
            ["--user", "show", RepositoryNames.UserServiceFile, "-p", "ExecStart", "--value"], capture: true );
        return result.StandardOutput.Contains( $"path={destination} ;", StringComparison.Ordinal );
    }

    private static async Task VerifyServiceExecutable( ToolContext context, string expectedHash )
    {
        await Task.Delay( DaemonRestartDelayMilliseconds, context.CancellationToken );
        if ( await InspectWayscriberService( context ) != WayscriberServiceState.Active )
        {
            throw new ToolException( "Updated Wayscriber service did not stay active." );
        }

        var result = await context.Run( Programs.SystemControl,
            ["--user", "show", RepositoryNames.UserServiceFile, "-p", "MainPID", "--value"], capture: true );
        if ( !int.TryParse( result.StandardOutput.Trim( ), out var pid ) || pid <= 0 ||
            Files.Sha256( $"/proc/{pid}/exe" ) != expectedHash )
        {
            throw new ToolException( "Wayscriber service did not run the selected app/broker cohort." );
        }
    }

    internal static async Task InstallAppCohortWithRestart( ToolContext context, string installDir, string destination,
        Func<ToolContext, Task> stopService, Func<ToolContext, Task> startService,
        Func<ToolContext, string, Task> verifyService )
    {
        if ( !File.Exists( destination ) )
        {
            throw new ToolException( $"Cannot restart an active service without its installed app at {destination}." );
        }

        var oldHash = Files.Sha256( destination );
        var oldLink = new FileInfo( destination ).LinkTarget;
        var backup = Path.Combine( installDir, $".wayscriber-previous-{Guid.NewGuid( ):N}" );
        var rollbackLink = Path.Combine( installDir, $".wayscriber-rollback-{Guid.NewGuid( ):N}" );
        var selected = await StageAppCohort( context, installDir );
        var newHash = Files.Sha256( Path.Combine( selected, RepositoryNames.MainPackage ) );
        var privileged = NeedsPrivilege( installDir );
        var recovery = context with { CancellationToken = CancellationToken.None };
        var stopping = false;
        var restoredOrInstalled = false;

        try
        {
            stopping = true;
            await stopService( context );
            if ( oldLink is null )
            {
                await RunPossiblyRoot( context, privileged, Programs.Move, ["-T", "--", destination, backup] );
            }

            await SelectAppCohort( context, installDir, destination, selected );
            await startService( context );
            await verifyService( context, newHash );
            restoredOrInstalled = true;
            await context.Output.WriteLineAsync( "Updated Wayscriber service is running the selected app/broker cohort." );
        }
        catch ( Exception installError )
        {
            if ( stopping )
            {
                try
                {
                    await stopService( recovery );
                    await RestoreAppSelector( recovery, privileged, destination, oldLink, oldHash, backup, rollbackLink );
                    await startService( recovery );
                    await verifyService( recovery, oldHash );
                    restoredOrInstalled = true;
                    await context.Error.WriteLineAsync( "Restored the previous Wayscriber selector and service after install failure." );
                }
                catch ( Exception rollbackError )
                {
                    throw new ToolException( $"Install failed: {installError.Message} Rollback failed: {rollbackError.Message} Previous app backup: {backup}" );
                }
            }
            throw;
        }
        finally
        {
            if ( restoredOrInstalled )
            {
                try
                {
                    await RunPossiblyRoot( recovery, privileged, Programs.Remove, ["-f", "--", backup, rollbackLink] );
                }
                catch ( Exception cleanupError )
                {
                    await context.Error.WriteLineAsync( $"Could not remove installer temporary files: {cleanupError.Message}" );
                }
            }
        }
    }

    private static async Task RestoreAppSelector( ToolContext context, bool privileged, string destination,
        string? oldLink, string oldHash, string backup, string rollbackLink )
    {
        if ( oldLink is not null )
        {
            await RunPossiblyRoot( context, privileged, Programs.Link, ["-s", oldLink, rollbackLink] );
            await RunPossiblyRoot( context, privileged, Programs.Move, ["-Tf", "--", rollbackLink, destination] );
        }
        else if ( File.Exists( backup ) )
        {
            await RunPossiblyRoot( context, privileged, Programs.Move, ["-Tf", "--", backup, destination] );
        }
        else if ( !File.Exists( destination ) || Files.Sha256( destination ) != oldHash )
        {
            throw new ToolException( $"Previous Wayscriber app is missing; backup path: {backup}" );
        }

        if ( Files.Sha256( destination ) != oldHash )
        {
            throw new ToolException( $"Previous Wayscriber app hash did not match after rollback: {destination}" );
        }
    }
}
