namespace Wayscriber.Tools.Tests;

// The checkout under test: the parent of the directory that holds the test entry point.
internal static class TestRepository
{
    public static string Root
    {
        get
        {
            var directory = AppContext.GetData( "EntryPointFileDirectoryPath" ) as string ?? Environment.CurrentDirectory;
            return Path.GetFullPath( Path.Combine( directory, ".." ) );
        }
    }
}
