import * as fs from 'fs';
import * as path from 'path';
import { spawnSync } from 'child_process';
import { exit } from 'process';

let exo_executable = process.env.EXO_EXECUTABLE || "";

if (!exo_executable) {
  const MODE = process.env.EXECUTABLE_MODE;

  switch (MODE) {
    case "debug":
      exo_executable = path.resolve(__dirname, "../../target/debug/exo");
      break;
    case "release":
      exo_executable = path.resolve(__dirname, "../../target/release/exo");
      break;
    default:
      console.error(`Unknown mode: ${MODE} for EXECUTABLE_MODE`);
      process.exit(1);
  }
}

function isExographProject(directory: string): boolean {
  const indexExoPath = path.join(directory, 'src', 'index.exo');
  return fs.existsSync(indexExoPath);
}

function exographProjects(startPath: string): string[] {
  let results: string[] = [];

  function searchDirectories(currentPath: string) {
    const filesAndDirs = fs.readdirSync(currentPath);

    for (const fileOrDir of filesAndDirs) {
      const fullPath = path.join(currentPath, fileOrDir);
      const stat = fs.statSync(fullPath);

      if (stat.isDirectory()) {
        if (isExographProject(fullPath)) {
          results.push(fullPath);
        }
        searchDirectories(fullPath);
      }
    }
  }

  searchDirectories(startPath);
  return results;
}

class Failure {
  constructor(readonly path: string, readonly reason: string, readonly diff: string) {
    this.path = path;
    this.reason = reason;
    this.diff = diff;
  }

  get actualErrorFilePath(): string {
    return path.join(this.path, 'error.txt.new');
  }

  get expectedErrorFilePath(): string {
    return path.join(this.path, 'error.txt');
  }
}

function checkExographProjects(directories: string[]): Array<Failure> {
  let failedProjects: Array<Failure> = [];

  directories.forEach(directory => {
    console.log("Checking", directory);
    const expectedErrorFilePath = path.join(directory, 'error.txt');
    const actualErrorPath = expectedErrorFilePath + ".new";

    const result = spawnSync(exo_executable, ['build'], {
      cwd: directory, stdio: 'pipe',
    });

    if (result.status != 0) {
      const actualErrors = result.stderr.toString();
      if (fs.existsSync(expectedErrorFilePath)) {
        let diff = diffFiles(expectedErrorFilePath, actualErrorPath);
        if (diff) {
          fs.writeFileSync(actualErrorPath, actualErrors, 'utf-8');
          failedProjects.push(new Failure(directory, "Errors do not match. Check error.txt.new.", diff))
        }
      } else {
        fs.writeFileSync(actualErrorPath, actualErrors, 'utf-8');
        failedProjects.push(new Failure(directory, "Expected error not found", ""))
      }
    } else {
      failedProjects.push(new Failure(directory, "Expected errors, but the project built successfully", ""))
    }
  });

  return failedProjects
}

function diffFiles(expectedFile: string, actualFile: string): string {
  const diff = spawnSync('diff', ["-b", expectedFile, actualFile], { encoding: 'utf-8', stdio: 'inherit' });
  return diff.stdout || "";
}

const exographDirectories = exographProjects('.');
const failed = checkExographProjects(exographDirectories);

if (failed.length == 0) {
  console.log("\x1b[32m%s\x1b[0m", "All tests passed!");
} else {
  // Sort failures by path name
  failed.sort((a, b) => a.path.localeCompare(b.path));

  console.log("The following tests failed:");
  failed.forEach(failure => {
    console.log("\x1b[31m%s\x1b[0m", `- ${failure.path}: ${failure.reason}`);
    if (failure.diff) {
      console.log("\x1b[33m%s\x1b[0m", `Diff between expected and actual error file for ${failure.path}:`);
      console.log(failure.diff);
    }
  });
  exit(1)
}

