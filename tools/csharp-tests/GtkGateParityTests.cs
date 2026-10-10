using System.Net.Sockets;
using System.Runtime.Versioning;
using Xunit;

namespace Wayscriber.Tools.Tests;

public sealed class GtkGateParityTests
{
    private const string PopupMarker = "EXECUTED: GTK native popup presentation regression";
    private const string MenuMarker = "EXECUTED: GTK native menu presentation assertions";
    private const string PenFeelMarker = "EXECUTED: GTK Pen feel fractional-scale render assertions";
    private const string FixtureSocket = "WAYSCRIBER_FIXTURE_SOCKET";
    private const string FixtureOutput = "WAYSCRIBER_FIXTURE_OUTPUT";
    private const string FixtureExit = "WAYSCRIBER_FIXTURE_EXIT";
    private const string FixtureRuntime = "WAYSCRIBER_FIXTURE_RUNTIME";
    private const string ShellProgram = "/usr/bin/bash";

    [Theory]
    [InlineData( null, ExitCodes.Success )]
    [InlineData( PopupMarker, ExitCodes.Success )]
    [InlineData( MenuMarker, ExitCodes.Success )]
    [InlineData( PenFeelMarker, ExitCodes.Success )]
    [InlineData( null, ExitCodes.Failure )]
    public async Task BothGatesRequireNativeAssertionsAndCleanUpTheirPrivateDisplay( string? omittedMarker, int childExitCode )
    {
        if ( !OperatingSystem.IsLinux( ) )
        {
            return;
        }

        using var fixture = new GtkGateFixture( omittedMarker, childExitCode );
        var csharp = await fixture.RunCsharp( );
        var csharpRuntime = File.ReadAllText( fixture.RuntimeLog );
        Assert.False( Directory.Exists( csharpRuntime ), "C# gate must clean up its private runtime" );

        var shell = await fixture.RunShell( );
        var shellRuntime = File.ReadAllText( fixture.RuntimeLog );
        Assert.False( Directory.Exists( shellRuntime ), "shell gate must clean up its private runtime" );
        Assert.NotEqual( csharpRuntime, shellRuntime );

        var expected = omittedMarker is null && childExitCode == ExitCodes.Success ? ExitCodes.Success : ExitCodes.Failure;
        Assert.Equal( expected, csharp.ExitCode );
        Assert.Equal( expected, shell.ExitCode );
        if ( omittedMarker is not null )
        {
            Assert.Contains( omittedMarker, csharp.StandardError, StringComparison.Ordinal );
        }
    }

    [SupportedOSPlatform( "linux" )]
    private sealed class GtkGateFixture : IProcessRunner, IDisposable
    {
        private readonly TemporaryDirectory _directory = new( "wayscriber-gtk-gate-parity" );
        private readonly Socket _socket = new( AddressFamily.Unix, SocketType.Stream, ProtocolType.Unspecified );
        private readonly ProcessRunner _runner = new( TextWriter.Null, TextWriter.Null );
        private readonly Dictionary<string, string?> _environment;
        private readonly string _bin;

        public GtkGateFixture( string? omittedMarker, int childExitCode )
        {
            _bin = Path.Combine( _directory.Path, "bin" );
            Directory.CreateDirectory( _bin );
            var tools = Path.Combine( _directory.Path, "tools" );
            Directory.CreateDirectory( tools );
            File.Copy( Path.Combine( TestRepository.Root, "tools/test-gtk-widgets.sh" ), Path.Combine( tools, "test-gtk-widgets.sh" ) );

            var socket = Path.Combine( _directory.Path, "socket" );
            _socket.Bind( new UnixDomainSocketEndPoint( socket ) );
            var output = Path.Combine( _directory.Path, "output" );
            string[] markers =
            [
                "EXECUTED: GTK focus and slider assertions",
                "EXECUTED: GTK widget contract assertions",
                PopupMarker,
                MenuMarker,
                PenFeelMarker,
            ];
            File.WriteAllLines( output, markers.Where( marker => marker != omittedMarker ) );
            RuntimeLog = Path.Combine( _directory.Path, "runtime" );
            _environment = new Dictionary<string, string?>
            {
                [EnvironmentVariables.Path] = _bin + Path.PathSeparator + Environment.GetEnvironmentVariable( EnvironmentVariables.Path ),
                [EnvironmentVariables.Display] = ":fixture-display-must-be-removed",
                [FixtureSocket] = socket,
                [FixtureOutput] = output,
                [FixtureExit] = childExitCode.ToString( System.Globalization.CultureInfo.InvariantCulture ),
                [FixtureRuntime] = RuntimeLog,
            };

            // These stand-ins only test gate admission and cleanup. Real GTK
            // presentation remains owned by the Rust tests on private Weston.
            WriteExecutable( "weston", $$"""
#!/usr/bin/bash
ln -s "${{{FixtureSocket}}:?}" "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY"
exec /usr/bin/sleep 30
""" );
            WriteExecutable( "dbus-run-session", $$"""
#!/usr/bin/bash
set -euo pipefail
[[ -z "${DISPLAY:-}" ]]
[[ "$GDK_BACKEND" == wayland ]]
[[ "$WAYSCRIBER_REQUIRE_GTK_TESTS" == 1 ]]
[[ -S "$XDG_RUNTIME_DIR/$WAYLAND_DISPLAY" ]]
printf '%s' "$XDG_RUNTIME_DIR" > "${{{FixtureRuntime}}:?}"
cat "${{{FixtureOutput}}:?}"
exit "${{{FixtureExit}}:?}"
""" );
        }

        public string RuntimeLog { get; }

        public async Task<ProcessResult> RunCsharp( )
        {
            using var output = new StringWriter( );
            using var error = new StringWriter( );
            var context = new ToolContext( _directory.Path, output, error, this, CancellationToken.None );
            var command = DevelopmentCommands.Commands.Single( item => item.Name == CommandNames.GtkWidgets );

            try
            {
                var result = await command.Handler( context, [] );
                return new ProcessResult( result, output.ToString( ), error.ToString( ) );
            }
            catch ( ToolException failure )
            {
                return new ProcessResult( failure.ExitCode, output.ToString( ), failure.Message );
            }
        }

        public Task<ProcessResult> RunShell( ) => _runner.RunAsync( new ProcessRequest(
            ShellProgram, [Path.Combine( _directory.Path, "tools/test-gtk-widgets.sh" )], _directory.Path,
            _environment, CaptureOutput: true, Trace: false, AllowedExitCodes: new HashSet<int> { ExitCodes.Success, ExitCodes.Failure } ),
            CancellationToken.None );

        public Task<ProcessResult> RunAsync( ProcessRequest request, CancellationToken cancellationToken )
        {
            var environment = new Dictionary<string, string?>( _environment );
            foreach ( var pair in request.Environment! )
            {
                environment[pair.Key] = pair.Value;
            }

            return _runner.RunAsync( request with { FileName = Path.Combine( _bin, request.FileName ), Environment = environment },
                cancellationToken );
        }

        public void Dispose( )
        {
            _socket.Dispose( );
            _directory.Dispose( );
        }

        private void WriteExecutable( string name, string content )
        {
            var path = Path.Combine( _bin, name );
            File.WriteAllText( path, content );
            File.SetUnixFileMode( path, UnixFileMode.UserRead | UnixFileMode.UserWrite | UnixFileMode.UserExecute );
        }
    }
}
