; Instalador de Crucible para Windows (Inno Setup 6).
;
; Se instala por usuario, sin pedir administrador: quien simula un banco suele
; estar en un equipo corporativo sin permisos de instalación. Por eso va a
; %LOCALAPPDATA%\Programs y no a Program Files, y el banco de ejemplo queda en
; una carpeta en la que el usuario puede escribir.
;
; Compilar (lo hace el workflow de release):
;   iscc /DVersion=0.1.0 /DBinario=..\..\target\release\crucible.exe crucible.iss

#ifndef Version
  #define Version "0.0.0"
#endif
#ifndef Binario
  #define Binario "..\..\target\release\crucible.exe"
#endif

[Setup]
AppId={{6F4B3E0A-5C1D-4B8E-9E2A-C7D1A0F3B512}
AppName=Crucible
AppVersion={#Version}
AppPublisher=ANLACO
AppPublisherURL=https://github.com/anlaco/Crucible
DefaultDirName={localappdata}\Programs\Crucible
DefaultGroupName=Crucible
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
ChangesEnvironment=yes
LicenseFile=..\..\LICENSE
OutputDir=..\..\dist
OutputBaseFilename=crucible-{#Version}-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayName=Crucible {#Version}

[Languages]
Name: "es"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "path"; Description: "Añadir el comando 'crucible' al PATH"; Flags: checkedonce

[Files]
Source: "{#Binario}"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\..\LICENSE"; DestDir: "{app}"; Flags: ignoreversion
; El banco no se sobrescribe al actualizar ni se borra al desinstalar: son los
; ficheros que el usuario edita para describir su banco.
Source: "..\..\banco\banco.yaml"; DestDir: "{app}\banco"; Flags: onlyifdoesntexist uninsneveruninstall
Source: "..\..\banco\perfiles\*.yaml"; DestDir: "{app}\banco\perfiles"; Flags: onlyifdoesntexist uninsneveruninstall

[InstallDelete]
; Versiones anteriores traían el manual en .md; al actualizar quedaría desfasado.
Type: files; Name: "{app}\MANUAL.md"
Type: files; Name: "{app}\banco\MANUAL.md"

[Icons]
; cmd /k deja la ventana abierta si crucible termina, para poder leer el motivo.
Name: "{group}\Arrancar banco"; Filename: "{cmd}"; Parameters: "/k ""{app}\crucible.exe"" ""{app}\banco\banco.yaml"""; WorkingDir: "{app}\banco"
Name: "{group}\Abrir carpeta del banco"; Filename: "{app}\banco"
Name: "{group}\Terminal de Crucible"; Filename: "{cmd}"; Parameters: "/k ""{app}\crucible.exe"" --ayuda"; WorkingDir: "{app}\banco"
; El manual vive en la web: una sola copia, comprobada contra el binario en
; cada publicación, en vez de un .md empaquetado que se desfasa.
Name: "{group}\Manual"; Filename: "https://anlaco.github.io/Crucible/"
Name: "{group}\Desinstalar Crucible"; Filename: "{uninstallexe}"

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; \
  ValueData: "{olddata};{app}"; Tasks: path; Check: NoEstaEnPath(ExpandConstant('{app}'))

[Run]
Filename: "{cmd}"; Parameters: "/k ""{app}\crucible.exe"" ""{app}\banco\banco.yaml"""; WorkingDir: "{app}\banco"; \
  Description: "Arrancar el banco de ejemplo"; Flags: postinstall nowait skipifsilent unchecked

[Code]
function NoEstaEnPath(Dir: string): Boolean;
var
  Actual: string;
begin
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Actual) then
  begin
    Result := True;
    exit;
  end;
  Result := Pos(';' + Uppercase(Dir) + ';', ';' + Uppercase(Actual) + ';') = 0;
end;

// Quita {app} del PATH al desinstalar, sin tocar el resto.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
var
  Actual, Dir: string;
  P: Integer;
begin
  if CurUninstallStep <> usPostUninstall then
    exit;
  if not RegQueryStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Actual) then
    exit;
  Dir := ExpandConstant('{app}');
  Actual := ';' + Actual + ';';
  P := Pos(';' + Uppercase(Dir) + ';', Uppercase(Actual));
  if P = 0 then
    exit;
  Delete(Actual, P, Length(Dir) + 1);
  Actual := Copy(Actual, 2, Length(Actual) - 2);
  RegWriteExpandStringValue(HKEY_CURRENT_USER, 'Environment', 'Path', Actual);
end;
