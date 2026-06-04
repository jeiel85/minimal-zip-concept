# register_sparse_package.ps1
# Windows 11 Modern Context Menu (Sparse Package) 등록/해제 자동화 스크립트
# Publisher CN=Antigravity 자체 서명 인증서 생성, 등록, MSIX 패키징, 서명 및 최종 AppxPackage 등록을 처리합니다.

param(
    [switch]$Uninstall
)

$ErrorActionPreference = "Stop"

# UTF-8 출력 강제
$OutputEncoding = [System.Text.Encoding]::UTF8
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8

$AppRoot = (Get-Item "$PSScriptRoot\..").FullName
$TargetDir = "$AppRoot\target"
$StagingDir = "$TargetDir\sparse_pkg"
$MsixPath = "$TargetDir\mzc_package.msix"
$CertPath = "$TargetDir\mzc_cert.cer"
$ManifestPath = "$AppRoot\AppxManifest.xml"

# 1. 패키지 이름 정의
$PackageFamilyName = "MinimalZipConcept.Mzc"
$PublisherCommonName = "Antigravity"

# --- 삭제 (Uninstall) 모드 ---
if ($Uninstall) {
    Write-Host "MZC Sparse Package 및 인증서 제거 작업을 시작합니다..." -ForegroundColor Cyan
    
    # AppxPackage 등록 해제
    $pkg = Get-AppxPackage -Name $PackageFamilyName
    if ($pkg) {
        Write-Host "기존 설치된 패키지 해제 중: $($pkg.PackageFullName)" -ForegroundColor Yellow
        Remove-AppxPackage -Package $pkg.PackageFullName
        Write-Host "AppxPackage 해제 완료." -ForegroundColor Green
    } else {
        Write-Host "등록된 MZC AppxPackage를 찾을 수 없습니다." -ForegroundColor Gray
    }

    # TrustedPeople 및 Personal 저장소에서 인증서 삭제
    $certs = Get-ChildItem "Cert:\CurrentUser\My", "Cert:\CurrentUser\TrustedPeople" | Where-Object { $_.Subject -like "*CN=$PublisherCommonName*" }
    foreach ($cert in $certs) {
        Write-Host "인증서 삭제 중: $($cert.Thumbprint) ($($cert.Subject))" -ForegroundColor Yellow
        Remove-Item $_.PSPath
    }
    
    # 임시 파일 정리
    if (Test-Path $MsixPath) { Remove-Item $MsixPath }
    if (Test-Path $CertPath) { Remove-Item $CertPath }
    if (Test-Path $StagingDir) { Remove-Item -Recurse -Force $StagingDir }
    
    Write-Host "MZC 제거 및 청소가 완료되었습니다." -ForegroundColor Green
    exit
}

# --- 설치 (Install) 모드 ---
Write-Host "MZC Sparse Package 빌드 및 등록을 시작합니다..." -ForegroundColor Cyan

# 0. 빌드 도구 검사 (makeappx.exe 및 signtool.exe 자동 검색)
$sdkPath = Join-Path ${env:ProgramFiles(x86)} "Windows Kits\10\bin"
if (-not (Test-Path $sdkPath)) {
    $sdkPath = "C:\Program Files (x86)\Windows Kits\10\bin"
}

$makeappx = $null
$signtool = $null

if (Test-Path $sdkPath) {
    # 최신 버전을 찾기 위해 역정렬
    $subdirs = Get-ChildItem $sdkPath | Where-Object { $_.PSIsContainer } | Sort-Object Name -Descending
    foreach ($dir in $subdirs) {
        $x64Path = Join-Path $dir.FullName "x64"
        if (Test-Path (Join-Path $x64Path "makeappx.exe")) {
            $makeappx = Join-Path $x64Path "makeappx.exe"
            $signtool = Join-Path $x64Path "signtool.exe"
            break
        }
    }
}

# 시스템 PATH에서도 검색 시도
if (-not $makeappx) { $makeappx = Get-Command "makeappx.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source }
if (-not $signtool) { $signtool = Get-Command "signtool.exe" -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source }

if (-not $makeappx -or -not $signtool) {
    Write-Host "에러: Windows SDK 도구(makeappx.exe 또는 signtool.exe)를 찾을 수 없습니다." -ForegroundColor Red
    Write-Host "Windows 10/11 SDK를 설치하거나 환경변수 PATH를 확인해 주세요." -ForegroundColor Yellow
    exit 1
}

Write-Host "사용할 SDK 도구:" -ForegroundColor Gray
Write-Host " - MakeAppx: $makeappx" -ForegroundColor Gray
Write-Host " - SignTool: $signtool" -ForegroundColor Gray

# 1. 빌드 파일 체크 (mzc.exe 가 존재하는지 검사)
$exePath = "$TargetDir\release\mzc.exe"
if (-not (Test-Path $exePath)) {
    Write-Host "경고: Release 빌드를 찾을 수 없습니다. Debug 빌드를 탐색합니다..." -ForegroundColor Yellow
    $exePath = "$TargetDir\debug\mzc.exe"
    if (-not (Test-Path $exePath)) {
        Write-Host "에러: mzc.exe가 빌드되어 있지 않습니다. 먼저 'cargo build --release'를 실행해 주세요." -ForegroundColor Red
        exit 1
    }
}
Write-Host "연동할 실행 파일: $exePath" -ForegroundColor Gray

# 2. 임시 Staging 디렉토리 생성 및 파일 복사
if (Test-Path $StagingDir) {
    Remove-Item -Recurse -Force $StagingDir
}
New-Item -ItemType Directory -Path $StagingDir | Out-Null
New-Item -ItemType Directory -Path "$StagingDir\assets" | Out-Null

Copy-Item $ManifestPath -Destination "$StagingDir\AppxManifest.xml"
Copy-Item $exePath -Destination "$StagingDir\mzc.exe"
Copy-Item "$AppRoot\assets\*" -Destination "$StagingDir\assets" -Recurse

# 3. MSIX 패키지 파일 생성
Write-Host "MakeAppx를 사용하여 MSIX 패키지를 생성하는 중..." -ForegroundColor Gray
if (Test-Path $MsixPath) { Remove-Item $MsixPath }

# makeappx pack /d <layout> /p <package> /o /nv
& $makeappx pack /d $StagingDir /p $MsixPath /o /nv | Out-Null
Write-Host "MSIX 패키지 생성 완료: $MsixPath" -ForegroundColor Green

# 4. 자체 서명 인증서 생성 및 등록
$subject = "CN=$PublisherCommonName"
Write-Host "인증서를 검사/생성 중: $subject" -ForegroundColor Gray

# 기존 인증서 검색
$cert = Get-ChildItem "Cert:\CurrentUser\My" | Where-Object { $_.Subject -eq $subject } | Select-Object -First 1

if (-not $cert) {
    Write-Host "인증서가 존재하지 않아 새로 생성합니다..." -ForegroundColor Yellow
    $cert = New-SelfSignedCertificate -Type Custom -Subject $subject `
        -KeyUsage DigitalSignature `
        -FriendlyName "MZC Sparse Package Certificate" `
        -CertStoreLocation "Cert:\CurrentUser\My" `
        -TextExtension @("2.5.29.37={text}1.3.6.1.5.5.7.3.3") # Code Signing OID
}

# 인증서 내보내기 및 신뢰할 수 있는 사용자(Trusted People) 저장소에 등록
Export-Certificate -Cert $cert -FilePath $CertPath | Out-Null
Write-Host "인증서 내보내기 완료: $CertPath" -ForegroundColor Gray

# 신뢰할 수 있는 사용자 저장소에 추가 (관리자 권한 없이 CurrentUser 범위 내에서 가능)
$trustedStore = "Cert:\CurrentUser\TrustedPeople"
$alreadyTrusted = Get-ChildItem $trustedStore | Where-Object { $_.Thumbprint -eq $cert.Thumbprint }
if (-not $alreadyTrusted) {
    Write-Host "신뢰할 수 있는 사용자(TrustedPeople) 저장소에 인증서를 등록합니다..." -ForegroundColor Yellow
    Import-Certificate -FilePath $CertPath -CertStoreLocation $trustedStore | Out-Null
}
Write-Host "인증서가 신뢰할 수 있는 목록에 등록되었습니다." -ForegroundColor Green

# 5. MSIX 패키지 서명
Write-Host "SignTool을 사용하여 패키지 서명 중..." -ForegroundColor Gray
# signtool sign /fd SHA256 /sha1 <thumbprint> <package>
& $signtool sign /fd SHA256 /sha1 $cert.Thumbprint $MsixPath | Out-Null
Write-Host "패키지 서명 완료." -ForegroundColor Green

# 6. Sparse Package 시스템 등록
Write-Host "Windows 11 탐색기에 MZC Sparse Package 등록 중..." -ForegroundColor Gray

# 기존 패키지가 등록되어 있으면 먼저 해제
$existingPkg = Get-AppxPackage -Name $PackageFamilyName
if ($existingPkg) {
    Write-Host "기존 등록된 패키지 재인스톨 해제 중..." -ForegroundColor Yellow
    Remove-AppxPackage -Package $existingPkg.PackageFullName | Out-Null
}

# 외부 소스(External Location)로 패키지 등록
Add-AppxPackage -Path $MsixPath -ExternalLocationPath $AppRoot

Write-Host "축하합니다! MZC Sparse Package 등록이 성공적으로 완료되었습니다." -ForegroundColor Green
Write-Host "이제 Windows 11 탐색기에서 파일/폴더 우클릭 시 모던 컨텍스트 메뉴(MZC로 압축/해제)가 바로 표시됩니다." -ForegroundColor Cyan
Write-Host " - 설치 경로: $AppRoot" -ForegroundColor Gray
