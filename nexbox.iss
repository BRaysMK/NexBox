; NexBox 安装脚本 - Inno Setup
; 新境盒 - 系统优化工具箱

#define MyAppName "NexBox"
#define MyAppDisplayName "新境盒"
#define MyAppVersion "4.2.6"
#define MyAppPublisher "MuLiu"
#define MyAppURL "https://www.nexbox.top/"
#define MyAppExeName "nexbox.exe"

[Setup]
AppId={{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppDisplayName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppDisplayName}
DisableProgramGroupPage=yes
OutputBaseFilename=NexBox_{#MyAppVersion}_Setup
SetupIconFile=D:\NexBox\src-tauri\icons\icon.ico
Compression=lzma2/ultra64
SolidCompression=yes
WizardStyle=modern
PrivilegesRequired=admin
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppDisplayName}

[Languages]
Name: "chinesesimplified"; MessagesFile: "D:\NexBox\ChineseSimplified.isl"

[CustomMessages]
chinesesimplified.WelcomeLabel1=欢迎安装 新境盒
chinesesimplified.WelcomeLabel2=本程序将在您的计算机上安装 新境盒 %n%n建议您在继续之前关闭所有其他应用程序。
chinesesimplified.WizardLicense=许可协议
chinesesimplified.LicenseLabel=请在安装前阅读以下重要信息。
chinesesimplified.LicenseLabel3=请在安装前阅读以下许可协议。您必须接受此协议才能继续安装。
chinesesimplified.LicenseAccepted=我接受协议(&A)
chinesesimplified.LicenseNotAccepted=我不接受协议(&D)
chinesesimplified.WizardSelectDir=选择安装位置
chinesesimplified.SelectDirLabel=请选择 新境盒 的安装位置。
chinesesimplified.SelectDirBrowseLabel=单击"下一步"继续。如果您想选择其他文件夹，请单击"浏览"。
chinesesimplified.DiskSpaceMBLabel=所需空间至少
chinesesimplified.SelectDirDesc=将 新境盒 安装到何处？
chinesesimplified.DirExists=文件夹 "%1" 已存在。%n%n您要安装到该文件夹吗？
chinesesimplified.DirDoesntExist=文件夹 "%1" 不存在。%n%n您要创建该文件夹吗？
chinesesimplified.WizardSelectTasks=选择附加任务
chinesesimplified.SelectTasksDesc=您想要执行哪些附加任务？
chinesesimplified.SelectTasksLabel=选择您想要在安装 新境盒 时执行的附加任务，然后单击"下一步"。
chinesesimplified.TasksDesktop=创建桌面快捷方式
chinesesimplified.WizardReady=准备安装
chinesesimplified.ReadyLabel=安装程序已准备好将 新境盒 安装到您的计算机。
chinesesimplified.ReadyLabel2=单击"安装"继续。如果您想检查或更改任何设置，请单击"上一步"。
chinesesimplified.ReadyMemoDir=安装位置：
chinesesimplified.ReadyMemoTasks=附加任务：
chinesesimplified.InstallingLabel=正在安装 新境盒，请稍候...
chinesesimplified.FinishedHeadingLabel=新境盒 安装完成
chinesesimplified.FinishedLabel=新境盒 已成功安装到您的计算机。
chinesesimplified.FinishedLabelNoIcons=新境盒 已成功安装到您的计算机。
chinesesimplified.ClickFinish=单击"完成"退出安装程序。
chinesesimplified.Run=运行 新境盒(&L)
chinesesimplified.UninstallProgram=卸载 新境盒
chinesesimplified.Uninstalling=正在卸载 新境盒...
chinesesimplified.UninstallAppTitle=卸载 新境盒
chinesesimplified.UninstallAppFullTitle=卸载 新境盒
chinesesimplified.UninstallSuccess=新境盒 已成功从您的计算机卸载。
chinesesimplified.UninstallNotFound=卸载失败。未找到卸载信息。
chinesesimplified.UninstallConfirm=您确定要完全删除 新境盒 及其所有组件吗？
chinesesimplified.BeveledLabel=新境盒 安装向导
chinesesimplified.BeveledLabelUninstall=新境盒 卸载向导
chinesesimplified.WizardPreparing=准备安装
chinesesimplified.PreparingDesc=安装程序正在准备将 新境盒 安装到您的计算机。
chinesesimplified.WizardInstalling=正在安装
chinesesimplified.Installing=正在安装 新境盒...
chinesesimplified.Extracting=正在解压文件...
chinesesimplified.CreatingUninstall=正在创建卸载程序...
chinesesimplified.CreatingIcons=正在创建快捷方式...
chinesesimplified.CreatingRegistry=正在写入注册表...
chinesesimplified.CreatingIni=正在创建配置文件...
chinesesimplified.StatusClosingApplications=正在关闭应用程序...
chinesesimplified.StatusRestartingApplications=正在重启应用程序...
chinesesimplified.StatusRollback=正在回滚更改...
chinesesimplified.ButtonNext=下一步(&N) >
chinesesimplified.ButtonBack=< 上一步(&B)
chinesesimplified.ButtonInstall=安装(&I)
chinesesimplified.ButtonFinish=完成(&F)
chinesesimplified.ButtonCancel=取消
chinesesimplified.ButtonBrowse=浏览(&B)...
chinesesimplified.ButtonYes=是(&Y)
chinesesimplified.ButtonNo=否(&N)
chinesesimplified.ButtonYesToAll=全是(&A)
chinesesimplified.ButtonNoToAll=全否(&O)
chinesesimplified.ButtonAbort=中止(&A)
chinesesimplified.ButtonRetry=重试(&R)
chinesesimplified.ButtonIgnore=忽略(&I)
chinesesimplified.ExitSetupTitle=退出安装
chinesesimplified.ExitSetupMessage=安装尚未完成。如果您现在退出，程序将不会安装。%n%n您可以稍后再次运行安装程序完成安装。%n%n退出安装吗？
chinesesimplified.SetupWindowTitle=新境盒 安装程序
chinesesimplified.UninstallWindowTitle=新境盒 卸载程序

[Tasks]
Name: "desktopicon"; Description: "{cm:TasksDesktop}"

[Files]
Source: "D:\NexBox\src-tauri\target\release\{#MyAppExeName}"; DestDir: "{app}"; Flags: ignoreversion
Source: "D:\NexBox\MiSans-Medium.ttf"; DestDir: "{app}"; Flags: ignoreversion
Source: "D:\NexBox\nvidiaProfileInspector.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "D:\NexBox\nvidiaProfileInspector.exe.config"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "D:\NexBox\Reference.xml"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "D:\NexBox\src-tauri\target\release\resources\*"; DestDir: "{app}\resources"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "D:\NexBox\src-tauri\target\release\assets\*"; DestDir: "{app}\assets"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "D:\NexBox\power-plans\*"; DestDir: "{app}\power-plans"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist
Source: "D:\NexBox\PawnIO_setup.exe"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "D:\NexBox\monitor\bin\Release\net48\*"; DestDir: "{app}\monitor"; Flags: ignoreversion recursesubdirs createallsubdirs skipifsourcedoesntexist

[Icons]
Name: "{autoprograms}\{#MyAppDisplayName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppDisplayName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:Run}"; Flags: nowait postinstall skipifsilent shellexec

[UninstallDelete]
Type: filesandordirs; Name: "{app}\resources"
Type: filesandordirs; Name: "{app}\assets"
Type: filesandordirs; Name: "{app}\power-plans"
Type: files; Name: "{app}\nvidiaProfileInspector.exe"
Type: files; Name: "{app}\nvidiaProfileInspector.exe.config"
Type: files; Name: "{app}\Reference.xml"
Type: files; Name: "{app}\MiSans-Medium.ttf"
Type: files; Name: "{app}\PawnIO_setup.exe"
Type: filesandordirs; Name: "{app}\monitor"

[Code]
function InitializeSetup(): Boolean;
begin
  Result := True;
end;
