using System.ComponentModel;
using System.Diagnostics;
using System.Text;

namespace Wayscriber.Tools;

internal sealed record ProcessRequest(
    string FileName,
    IReadOnlyList<string> Arguments,
    string WorkingDirectory,
    IReadOnlyDictionary<string, string?>? Environment = null,
    bool CaptureOutput = false,
    string? StandardInput = null,
    bool Trace = true,
    IReadOnlySet<int>? AllowedExitCodes = null );

internal sealed record ProcessResult( int ExitCode, string StandardOutput, string StandardError )
{
    public bool IsSuccess => ExitCode == ExitCodes.Success;
}

internal interface IProcessRunner
{
    Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken );
}

internal sealed class ProcessRunner( TextWriter output, TextWriter error ) : IProcessRunner
{
    private const int MaximumDiagnosticCharacters = 32_768;

    public async Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
    {
        if ( request.Trace )
        {
            await output.WriteLineAsync( $"+ {FormatCommand( request.FileName, request.Arguments )}", cancellationToken );
        }

        using var process = new Process { StartInfo = CreateStartInfo( request ) };
        try
        {
            if ( !process.Start( ) )
            {
                throw new ToolException( $"Could not start {request.FileName}.", ExitCodes.CommandNotFound );
            }

            Task<string>? stdout = request.CaptureOutput ? process.StandardOutput.ReadToEndAsync( cancellationToken ) : null;
            Task<string>? stderr = request.CaptureOutput ? process.StandardError.ReadToEndAsync( cancellationToken ) : null;
            if ( request.StandardInput is not null )
            {
                await process.StandardInput.WriteAsync( request.StandardInput.AsMemory( ), cancellationToken );
                process.StandardInput.Close( );
            }

            try
            {
                await process.WaitForExitAsync( cancellationToken );
            }
            catch ( OperationCanceledException )
            {
                TryTerminate( process );
                throw;
            }

            var result = new ProcessResult(
                process.ExitCode,
                stdout is null ? string.Empty : await stdout,
                stderr is null ? string.Empty : await stderr );
            if ( request.AllowedExitCodes?.Contains( result.ExitCode ) != true && !result.IsSuccess )
            {
                if ( request.CaptureOutput && result.StandardError.Length > 0 )
                {
                    await error.WriteAsync( Bound( result.StandardError ) );
                }
                throw new ToolException( $"{request.FileName} exited with code {result.ExitCode}.", result.ExitCode );
            }
            return result;
        }
        catch ( Win32Exception error )
        {
            throw new ToolException( $"Cannot start {request.FileName}: {error.Message}", ExitCodes.CommandNotFound );
        }
    }

    private static ProcessStartInfo CreateStartInfo( ProcessRequest request )
    {
        var startInfo = new ProcessStartInfo
        {
            FileName = request.FileName,
            WorkingDirectory = request.WorkingDirectory,
            UseShellExecute = false,
            RedirectStandardOutput = request.CaptureOutput,
            RedirectStandardError = request.CaptureOutput,
            RedirectStandardInput = request.StandardInput is not null,
        };
        foreach ( var argument in request.Arguments )
        {
            startInfo.ArgumentList.Add( argument );
        }
        if ( request.Environment is not null )
        {
            foreach ( var pair in request.Environment )
            {
                if ( pair.Value is null )
                {
                    startInfo.Environment.Remove( pair.Key );
                }
                else
                {
                    startInfo.Environment[pair.Key] = pair.Value;
                }
            }
        }
        return startInfo;
    }

    internal static string FormatCommand( string fileName, IReadOnlyList<string> arguments ) =>
        string.Join( ' ', new[] { fileName }.Concat( arguments.Select( Quote ) ) );

    private static string Quote( string value ) =>
        value.Length > 0 && value.All( character => char.IsLetterOrDigit( character ) || "-._/:=@".Contains( character ) )
            ? value
            : $"'{value.Replace( "'", "'\\''", StringComparison.Ordinal )}'";

    private static string Bound( string value ) => value.Length <= MaximumDiagnosticCharacters ? value : value[^MaximumDiagnosticCharacters..];

    private static void TryTerminate( Process process )
    {
        try
        {
            if ( !process.HasExited )
            {
                process.Kill( entireProcessTree: true );
            }
        }
        catch ( InvalidOperationException )
        {
        }
    }
}

internal static class ProcessExtensions
{
    public static Task<ProcessResult> Run(
        this ToolContext context,
        string fileName,
        IReadOnlyList<string> arguments,
        string? workingDirectory = null,
        IReadOnlyDictionary<string, string?>? environment = null,
        bool capture = false,
        string? input = null,
        bool trace = true,
        IReadOnlySet<int>? allowedExitCodes = null ) =>
        context.Processes.RunAsync(
            new ProcessRequest( fileName, arguments, workingDirectory ?? context.RepositoryRoot, environment, capture, input, trace, allowedExitCodes ),
            context.CancellationToken );
}
