#!/usr/bin/env dotnet
#:package YamlDotNet@16.3.0
#:include csharp/includes.cs

using Wayscriber.Tools;

if ( Environment.GetEnvironmentVariable( EnvironmentVariables.WayscriberSshAskpass ) == EnvironmentVariables.Enabled )
{
    Console.WriteLine( Environment.GetEnvironmentVariable( EnvironmentVariables.AurSshPassphrase ) ?? string.Empty );
    return ExitCodes.Success;
}

return await ToolApplication.RunAsync( args );
