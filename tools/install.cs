#!/usr/bin/env -S dotnet run --disable-build-servers --file
#:package YamlDotNet@16.3.0
#:include csharp/includes.cs

using Wayscriber.Tools;

if ( args is [CommandNames.Help or CommandLineOptions.ShortHelp or CommandLineOptions.Help] )
{
    return await ToolApplication.RunAsync( [CommandNames.Help, CommandAreas.Install] );
}

var explicitCommand = args.Length > 0 && args[0] is CommandNames.App or CommandNames.Configurator;
var command = explicitCommand ? args[0] : CommandNames.App;
var commandArguments = explicitCommand ? args[1..] : args;
return await ToolApplication.RunAsync( [CommandAreas.Install, command, .. commandArguments] );
