param(
    [ValidateSet("Fast", "Full")]
    [string]$Mode = "Full",

    [switch]$SkipReleaseBuild,
    [switch]$SkipWasm,
    [switch]$SkipSmoke
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$repoRoot = (Resolve-Path (Join-Path $PSScriptRoot "..")).Path
Set-Location $repoRoot

$createdPaths = @(
    "samples/repeated.release-test.mzc",
    "samples/repeated.release-test.restored.txt",
    "samples.release-test.mzc",
    "samples.release-test.out"
)

function Invoke-Step {
    param(
        [string]$Name,
        [scriptblock]$Action
    )

    Write-Host ""
    Write-Host "==> $Name"
    & $Action
}

function Invoke-Cargo {
    param([Parameter(ValueFromRemainingArguments = $true)][string[]]$Args)

    & cargo @Args
    if ($LASTEXITCODE -ne 0) {
        throw "cargo $($Args -join ' ') failed with exit code $LASTEXITCODE"
    }
}

function Remove-SmokeArtifacts {
    foreach ($path in $createdPaths) {
        if (Test-Path -LiteralPath $path) {
            Remove-Item -LiteralPath $path -Recurse -Force
        }
    }
}

function Get-CargoPackageVersion {
    $cargoToml = Get-Content -LiteralPath "Cargo.toml" -Raw
    if ($cargoToml -notmatch '(?m)^version\s*=\s*"([^"]+)"') {
        throw "Could not read package version from Cargo.toml"
    }
    return $Matches[1]
}

try {
    Remove-SmokeArtifacts

    Invoke-Step "cargo test --lib" {
        Invoke-Cargo test --lib
    }

    $testTargets = if ($Mode -eq "Fast") {
        @("archive_tests", "format_tests", "robustness_tests")
    } else {
        @(
            "roundtrip_tests",
            "archive_tests",
            "format_tests",
            "robustness_tests",
            "advanced_tests",
            "property_tests"
        )
    }

    foreach ($target in $testTargets) {
        Invoke-Step "cargo test --test $target" {
            Invoke-Cargo test --test $target
        }
    }

    if (-not $SkipReleaseBuild) {
        Invoke-Step "cargo build --release" {
            Invoke-Cargo build --release
        }
    }

    if (-not $SkipWasm) {
        Invoke-Step "cargo rustc wasm cdylib" {
            Invoke-Cargo rustc --lib --target wasm32-unknown-unknown --release --crate-type cdylib
            if (-not (Test-Path -LiteralPath "target/wasm32-unknown-unknown/release/mzc.wasm")) {
                throw "WASM build did not produce target/wasm32-unknown-unknown/release/mzc.wasm"
            }
        }
    }

    Invoke-Step "CLI version matches Cargo.toml" {
        $expected = Get-CargoPackageVersion
        $versionOutput = (& cargo run --quiet -- --version)
        if ($LASTEXITCODE -ne 0) {
            throw "cargo run -- --version failed with exit code $LASTEXITCODE"
        }
        if ($versionOutput -notmatch [regex]::Escape($expected)) {
            throw "CLI version '$versionOutput' does not include Cargo.toml version '$expected'"
        }
        Write-Host $versionOutput
    }

    if (-not $SkipSmoke) {
        Invoke-Step "file compress/inspect/decompress smoke" {
            Invoke-Cargo run --quiet -- compress samples/repeated.txt samples/repeated.release-test.mzc
            Invoke-Cargo run --quiet -- inspect samples/repeated.release-test.mzc
            Invoke-Cargo run --quiet -- decompress samples/repeated.release-test.mzc samples/repeated.release-test.restored.txt

            $sourceHash = (Get-FileHash -LiteralPath "samples/repeated.txt").Hash
            $restoredHash = (Get-FileHash -LiteralPath "samples/repeated.release-test.restored.txt").Hash
            if ($sourceHash -ne $restoredHash) {
                throw "File smoke roundtrip hash mismatch"
            }

            Remove-Item -LiteralPath "samples/repeated.release-test.mzc" -Force
            Remove-Item -LiteralPath "samples/repeated.release-test.restored.txt" -Force
        }

        Invoke-Step "archive compress/inspect/decompress smoke" {
            Invoke-Cargo run --quiet -- compress samples samples.release-test.mzc
            Invoke-Cargo run --quiet -- inspect samples.release-test.mzc
            Invoke-Cargo run --quiet -- decompress samples.release-test.mzc samples.release-test.out

            if (-not (Test-Path -LiteralPath "samples.release-test.out")) {
                throw "Archive smoke did not create samples.release-test.out"
            }
        }
    }

    Write-Host ""
    Write-Host "Release verification completed successfully ($Mode mode)."
} finally {
    Remove-SmokeArtifacts
}
