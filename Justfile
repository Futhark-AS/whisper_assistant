# Version bumping

# Bump patch version (0.1.6 -> 0.1.7)
patch:
    #!/usr/bin/env python3
    import re
    from pathlib import Path
    
    def bump(version, part):
        major, minor, patch = map(int, version.split('.'))
        if part == 'patch': patch += 1
        elif part == 'minor': minor += 1; patch = 0
        elif part == 'major': major += 1; minor = 0; patch = 0
        return f"{major}.{minor}.{patch}"
    
    pyproject = Path("pyproject.toml")
    init = Path("src/quedo/__init__.py")
    
    # Get current version
    content = pyproject.read_text()
    match = re.search(r'version = "(.+?)"', content)
    old = match.group(1)
    new = bump(old, "patch")
    
    # Update both files
    pyproject.write_text(content.replace(f'version = "{old}"', f'version = "{new}"'))
    init.write_text(init.read_text().replace(f'__version__ = "{old}"', f'__version__ = "{new}"'))
    print(f"Bumped {old} -> {new}")

# Bump minor version (0.1.6 -> 0.2.0)
minor:
    #!/usr/bin/env python3
    import re
    from pathlib import Path
    
    def bump(version, part):
        major, minor, patch = map(int, version.split('.'))
        if part == 'patch': patch += 1
        elif part == 'minor': minor += 1; patch = 0
        elif part == 'major': major += 1; minor = 0; patch = 0
        return f"{major}.{minor}.{patch}"
    
    pyproject = Path("pyproject.toml")
    init = Path("src/quedo/__init__.py")
    
    content = pyproject.read_text()
    match = re.search(r'version = "(.+?)"', content)
    old = match.group(1)
    new = bump(old, "minor")
    
    pyproject.write_text(content.replace(f'version = "{old}"', f'version = "{new}"'))
    init.write_text(init.read_text().replace(f'__version__ = "{old}"', f'__version__ = "{new}"'))
    print(f"Bumped {old} -> {new}")

# Bump major version (0.1.6 -> 1.0.0)
major:
    #!/usr/bin/env python3
    import re
    from pathlib import Path
    
    def bump(version, part):
        major, minor, patch = map(int, version.split('.'))
        if part == 'patch': patch += 1
        elif part == 'minor': minor += 1; patch = 0
        elif part == 'major': major += 1; minor = 0; patch = 0
        return f"{major}.{minor}.{patch}"
    
    pyproject = Path("pyproject.toml")
    init = Path("src/quedo/__init__.py")
    
    content = pyproject.read_text()
    match = re.search(r'version = "(.+?)"', content)
    old = match.group(1)
    new = bump(old, "major")
    
    pyproject.write_text(content.replace(f'version = "{old}"', f'version = "{new}"'))
    init.write_text(init.read_text().replace(f'__version__ = "{old}"', f'__version__ = "{new}"'))
    print(f"Bumped {old} -> {new}")
