# Gradle Golden Test Data

This directory contains test data for validating Gradle build file parsing (`build.gradle` and `build.gradle.kts`).

## Current Implementation Status

### ✅ **Supported Patterns (Working)**

The current Rust implementation uses a custom token-based parser and supports these dependency declaration formats:

1. **Pattern 2 - String Notation**:

   ```groovy
   compile 'org.apache.commons:commons-text:1.1'
   ```

2. **Pattern 3 - Parentheses Notation**:

   ```groovy
   implementation("com.example:library:1.0.0")
   ```

3. **Pattern 4 - Named Parameters**:

   ```groovy
   api group: 'com.google.guava', name: 'guava', version: '30.1-jre'
   ```

4. **Pattern 1 - Map Format with Brackets**:

   ```groovy
   runtimeOnly(
       [group: 'org.jacoco', name: 'org.jacoco.ant', version: '0.7.4.201502262128'],
       [group: 'org.jacoco', name: 'org.jacoco.agent', version: '0.7.4.201502262128']
   )
   ```

5. **Nested Function Calls**:

   ```groovy
   implementation(enforcedPlatform("com.fasterxml.jackson:jackson-bom:2.12.2"))
   ```

6. **Project References**:

   ```kotlin
   "testImplementation"(project(":utils:test-utils"))
   ```

7. **Quoted Kotlin DSL configuration names**:

   ```kotlin
   "implementation"("io.ktor:ktor-client-core:$ktorVersion")
   ```

8. **Limited malformed-string recovery**:

   ```groovy
   implementation "com.fasterxml.jackson:jackson-bom:2.12.2'
   ```

9. **Multiple `dependencies {}` blocks in one build file**:

   ```groovy
   dependencies {
       implementation 'org.scala-lang:scala-library:2.11.12'
   }

   dependencies {
       testImplementation 'junit:junit:4.13'
   }
   ```

### ⚠️ **Still Partial / Unsupported**

The Python ScanCode Toolkit implementation uses **pygmars** (a token-based parser with grammar) and still handles some richer cases that our parser does not fully resolve yet:

1. **Pattern 5 - Variable References**:

   ```groovy
   api dependencies.lombok
   ```

   - **Why unsupported**: Requires resolving variable references, which needs semantic analysis beyond regex

2. **String Interpolation Resolution** (Kotlin DSL):

   ```kotlin
   implementation("org.jetbrains.kotlin:kotlin-stdlib-jdk8:$kotlinPluginVersion")
   ```

   - **Why unsupported**: Requires variable resolution which isn't possible with static analysis

   - **Why partial**: We preserve the literal tokenized value, but do not resolve variables or version catalogs semantically

3. **Gradle dotted identifier handling**:

   ```groovy
   implementation libs.androidx.appcompat
   ```

   - **Why partial**: TOML-backed `libs.versions.toml` aliases can now be resolved from nearby catalogs, and committed Kotlin `buildSrc` object constants such as `Deps.AndroidX.core` can be resolved statically when the supporting files are present nearby. Unresolved arbitrary dotted identifiers (for example `dependencies.lombok`) still remain intentionally unsupported.

## Test Files Status

### Golden Test Coverage

- No parser-only Gradle goldens remain ignored.
- The cleanup now exercises `groovy4`, `groovy-no-parens`, `kotlin2`, and `end2end` directly instead of masking them behind `#[ignore]`.
- Additional parser goldens now cover TOML-backed version catalog alias resolution, committed `buildSrc` Kotlin constant resolution, Kotlin DSL quoted configuration names, and Gradle POM license metadata extraction.
- Remaining failures, if any, should now represent real parser regressions rather than deferred fixtures.

## Path Forward: Full Feature Parity

To achieve 100% feature parity with Python ScanCode Toolkit, the Rust implementation needs:

### Option 1: Tree-Sitter Integration (Recommended)

- **Dependencies already added**: `tree-sitter`, `tree-sitter-groovy`, `tree-sitter-kotlin`
- **Approach**: Parse build files into AST, walk the tree to extract dependencies
- **Benefits**:
  - Handles all 5 patterns correctly
  - Robust to formatting variations
  - Future-proof for additional patterns
- **Effort**: Medium (2-3 days implementation)

### Option 2: Token-Based Parser (Python's Approach)

- **Approach**: Implement a pygmars-style token parser with grammar rules
- **Benefits**: Exact parity with Python implementation
- **Drawbacks**: Significant complexity, reinventing the wheel
- **Effort**: High (5+ days implementation)

### Option 3: Enhanced Regex + Manual Parsing

- **Approach**: Add specialized regex for Pattern 1 and Pattern 5, skip interpolation
- **Benefits**: Incremental improvement
- **Drawbacks**: Still incomplete, brittle
- **Effort**: Low (1 day) but incomplete solution

## Recommendation

**For production use**, implement **Option 1 (Tree-Sitter)**. The dependencies are already in place, and tree-sitter grammars exist for both Groovy and Kotlin. This provides the most robust, maintainable solution with full feature parity.

**Current status** is suitable for:

- Common Gradle dependency declarations across Groovy and Kotlin DSL
- Map, nested-function, and project-reference extraction without code execution
- CI-backed regression checking without parser-only ignored goldens

## Python Reference Implementation

Location: `../../reference/scancode-toolkit/src/packagedcode/build_gradle.py`

Key differences:

- **Python**: Uses pygmars token parser with grammar rules (lines 77-92)
- **Rust**: Uses a custom token parser with broad syntactic coverage, but without full semantic resolution of variables and catalogs

Grammar used by Python:

```python
grammar = """
    DEPENDENCY-1: {<PACKAGE-IDENTIFIER>{3} <OPERATOR>}    # Map format
    DEPENDENCY-2: {<NAME> <TEXT> <LIT-STRING> <TEXT>}    # String notation (✅ supported)
    DEPENDENCY-3: {<NAME> <TEXT>? <OPERATOR> <LIT-STRING> <OPERATOR>}  # Parentheses (✅ supported)
    DEPENDENCY-4: {<NAME> <TEXT> <NAME-LABEL> <TEXT> <LIT-STRING> <PACKAGE-IDENTIFIER> <PACKAGE-IDENTIFIER> <OPERATOR>? <TEXT>}  # Named params (✅ supported)
    DEPENDENCY-5: {<NAME> <TEXT> <NAME> <OPERATOR> <NAME-ATTRIBUTE>}  # Variable refs (❌ unsupported)
"""
```

## Parser-Only License Comparison Boundary

Gradle parser goldens still use the shared parser-only comparator, which relaxes some license-related fields and nested match metadata. That means these fixtures remain most useful for validating dependency extraction and stable package metadata shape; suite-wide tightening of license assertions should happen in a dedicated follow-up once the shared comparator policy is revisited.
