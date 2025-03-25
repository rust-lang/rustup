if ($"{cargo_bin}" not-in ($env.Path | split row (char esep))) {
  $env.Path = ($env.Path | prepend $"{cargo_bin}")
}
