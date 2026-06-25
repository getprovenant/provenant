// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

use super::*;
use tempfile::tempdir;

#[test]
fn test_is_match() {
    assert!(GradleParser::is_match(Path::new("build.gradle")));
    assert!(GradleParser::is_match(Path::new("build.gradle.kts")));
    assert!(GradleParser::is_match(Path::new("project/build.gradle")));
    assert!(!GradleParser::is_match(Path::new("build.xml")));
    assert!(!GradleParser::is_match(Path::new("settings.gradle")));
}

#[test]
fn test_extract_simple_dependencies() {
    let content = r#"
dependencies {
    compile 'org.apache.commons:commons-text:1.1'
    testCompile 'junit:junit:4.12'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);

    let dep1 = &deps[0];
    assert_eq!(
        dep1.purl,
        Some("pkg:maven/org.apache.commons/commons-text@1.1".to_string())
    );
    assert_eq!(dep1.scope, Some("compile".to_string()));
    assert_eq!(dep1.is_runtime, Some(true));
    assert_eq!(dep1.is_pinned, Some(true));

    let dep2 = &deps[1];
    assert_eq!(dep2.purl, Some("pkg:maven/junit/junit@4.12".to_string()));
    assert_eq!(dep2.scope, Some("testCompile".to_string()));
    assert_eq!(dep2.is_runtime, Some(false));
    assert_eq!(dep2.is_optional, Some(true));
}

#[test]
fn test_extract_parens_notation() {
    let content = r#"
dependencies {
    implementation("com.example:library:1.0.0")
    testImplementation("junit:junit:4.13")
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.example/library@1.0.0".to_string())
    );
}

#[test]
fn test_extract_named_parameters() {
    let content = r#"
dependencies {
    api group: 'com.google.guava', name: 'guava', version: '30.1-jre'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.google.guava/guava@30.1-jre".to_string())
    );
    assert_eq!(deps[0].scope, Some("api".to_string()));
}

#[test]
fn test_multiple_dependency_blocks_all_parsed() {
    let content = r#"
dependencies {
    implementation 'org.scala-lang:scala-library:2.11.12'
}

dependencies {
    implementation 'commons-collections:commons-collections:3.2.2'
    testImplementation 'junit:junit:4.13'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 3);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/org.scala-lang/scala-library@2.11.12".to_string())
    );
    assert_eq!(
        deps[1].purl,
        Some("pkg:maven/commons-collections/commons-collections@3.2.2".to_string())
    );
    assert_eq!(deps[2].purl, Some("pkg:maven/junit/junit@4.13".to_string()));
    assert_eq!(deps[2].scope, Some("testImplementation".to_string()));
}

#[test]
fn test_nested_dependency_blocks_all_parsed() {
    let content = r#"
buildscript {
    dependencies {
        classpath("org.eclipse.jgit:org.eclipse.jgit:$jgitVersion")
    }
}

subprojects {
    dependencies {
        implementation("org.jetbrains.kotlin:kotlin-stdlib-jdk8:$kotlinPluginVersion")
    }
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);

    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/org.eclipse.jgit/org.eclipse.jgit@%24jgitVersion".to_string())
    );
    assert_eq!(deps[0].scope, Some("classpath".to_string()));
    assert_eq!(
        deps[1].purl,
        Some(
            "pkg:maven/org.jetbrains.kotlin/kotlin-stdlib-jdk8@%24kotlinPluginVersion".to_string()
        )
    );
    assert_eq!(deps[1].scope, Some("implementation".to_string()));
}

#[test]
fn test_no_version() {
    let content = r#"
dependencies {
    compile 'org.example:library'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].is_pinned, Some(false));
    assert_eq!(deps[0].extracted_requirement, Some("".to_string()));
}

#[test]
fn test_nested_function_calls() {
    let content = r#"
dependencies {
    implementation(enforcedPlatform("com.fasterxml.jackson:jackson-bom:2.12.2"))
    testImplementation(platform("org.junit:junit-bom:5.7.2"))
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.fasterxml.jackson/jackson-bom@2.12.2".to_string())
    );
    assert_eq!(deps[0].scope, Some("enforcedPlatform".to_string()));
    assert_eq!(deps[1].scope, Some("platform".to_string()));
}

#[test]
fn test_map_format() {
    let content = r#"
dependencies {
    runtimeOnly(
        [group: 'org.jacoco', name: 'org.jacoco.ant', version: '0.7.4.201502262128'],
        [group: 'org.jacoco', name: 'org.jacoco.agent', version: '0.7.4.201502262128']
    )
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].scope, Some("".to_string()));
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/org.jacoco.ant@0.7.4.201502262128".to_string())
    );
}

#[test]
fn test_bracket_map_dedupes_exact_string_overlap() {
    let content = r#"
dependencies {
    runtimeOnly 'org.springframework:spring-core:2.5',
            'org.springframework:spring-aop:2.5'
    runtimeOnly(
        [group: 'org.springframework', name: 'spring-core', version: '2.5'],
        [group: 'org.springframework', name: 'spring-aop', version: '2.5']
    )
}
"#;

    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/org.springframework/spring-core@2.5".to_string())
    );
    assert_eq!(
        deps[1].purl,
        Some("pkg:maven/org.springframework/spring-aop@2.5".to_string())
    );
}

#[test]
fn test_malformed_string_stops_cascading_false_positives() {
    let content = r#"
dependencies {
    implementation "com.fasterxml.jackson:jackson-bom:2.12.2'
    implementation" com.fasterxml.jackson.core:jackson-core"
    testImplementation 'org.junit:junit-bom:5.7.2'"
    testImplementation "org.junit.platform:junit-platform-commons"
}
"#;

    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.fasterxml.jackson/jackson-bom@2.12.2%27".to_string())
    );
}

#[test]
fn test_project_references() {
    let content = r#"
dependencies {
    implementation(project(":documentation"))
    implementation(project(":basics"))
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].scope, Some("implementation".to_string()));
    assert_eq!(
        deps[0]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("documentation")
    );
    assert_eq!(deps[0].purl, Some("pkg:maven/documentation".to_string()));
    assert_eq!(deps[1].scope, Some("implementation".to_string()));
    assert_eq!(
        deps[1]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("basics")
    );
    assert_eq!(deps[1].purl, Some("pkg:maven/basics".to_string()));
}

#[test]
fn test_nested_project_references_preserve_parent_path() {
    let content = r#"
dependencies {
    implementation(project(":libs:download"))
    implementation(project(":libs:index"))
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);

    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].purl, Some("pkg:maven/libs/download".to_string()));
    assert_eq!(deps[0].scope, Some("implementation".to_string()));
    assert_eq!(
        deps[0]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("libs:download")
    );
    assert_eq!(deps[1].scope, Some("implementation".to_string()));
    assert_eq!(
        deps[1]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("libs:index")
    );
    assert_eq!(deps[1].purl, Some("pkg:maven/libs/index".to_string()));
}

#[test]
fn test_testimplementation_project_reference_is_not_runtime() {
    let content = r#"
dependencies {
    testImplementation project(':mockito-config')
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].scope, Some("testImplementation".to_string()));
    assert_eq!(deps[0].purl, Some("pkg:maven/mockito-config".to_string()));
    assert_eq!(deps[0].is_runtime, Some(false));
    assert_eq!(deps[0].is_optional, Some(true));
    assert_eq!(
        deps[0]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("mockito-config")
    );
}

#[test]
fn test_unresolved_dotted_identifiers_are_ignored_but_project_refs_survive() {
    let content = r#"
dependencies {
    implementation Deps.AndroidX.core
    implementation Deps.AndroidX.androidxAnnotation
    testImplementation TestDeps.mockitoCore3
    testImplementation project(':mockito-config')
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);

    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].scope, Some("testImplementation".to_string()));
    assert_eq!(deps[0].purl, Some("pkg:maven/mockito-config".to_string()));
    assert_eq!(deps[0].is_runtime, Some(false));
    assert_eq!(deps[0].is_optional, Some(true));
    assert_eq!(
        deps[0]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("mockito-config")
    );
}

#[test]
fn test_buildsrc_kotlin_constants_resolve_from_committed_files() {
    let temp_dir = tempdir().unwrap();
    let build_src_dir = temp_dir
        .path()
        .join("buildSrc/src/main/java/com/example/buildsrc");
    std::fs::create_dir_all(&build_src_dir).unwrap();
    std::fs::write(
        build_src_dir.join("GradleDeps.kt"),
        r#"
object GradleDeps {
    object Kotlin {
        const val version = "2.0.0"
        const val gradlePlugin = "org.jetbrains.kotlin:kotlin-gradle-plugin:$version"
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        build_src_dir.join("Deps.kt"),
        r#"
object Deps {
    object AndroidX {
        const val core = "androidx.core:core:1.15.0"
    }

    object SoLoader {
        private const val version = "0.11.0"
        const val soloader = "com.facebook.soloader:soloader:$version"
    }
}
"#,
    )
    .unwrap();
    std::fs::write(
        build_src_dir.join("TestDeps.kt"),
        r#"
object TestDeps {
    const val junit = "junit:junit:4.13.2"
}
"#,
    )
    .unwrap();

    let build_gradle = temp_dir.path().join("build.gradle");
    std::fs::write(
        &build_gradle,
        r#"
buildscript {
    dependencies {
        classpath GradleDeps.Kotlin.gradlePlugin
    }
}

dependencies {
    implementation Deps.AndroidX.core
    implementation Deps.SoLoader.soloader
    implementation project(':fbcore')
    testImplementation(TestDeps.junit) {
        because 'exercise parenthesized symbolic refs'
    }
}
"#,
    )
    .unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);

    assert_eq!(package_data.dependencies.len(), 5);
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref()
            == Some("pkg:maven/org.jetbrains.kotlin/kotlin-gradle-plugin@2.0.0")
            && dependency.scope.as_deref() == Some("classpath")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/androidx.core/core@1.15.0")
            && dependency.scope.as_deref() == Some("implementation")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/com.facebook.soloader/soloader@0.11.0")
            && dependency.scope.as_deref() == Some("implementation")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/fbcore")
            && dependency.scope.as_deref() == Some("implementation")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/junit/junit@4.13.2")
            && dependency.scope.as_deref() == Some("testImplementation")
            && dependency.is_runtime == Some(false)
            && dependency.is_optional == Some(true)
    }));
}

#[test]
fn test_gradle_properties_and_local_assignments_resolve_interpolation() {
    let temp_dir = tempdir().unwrap();
    std::fs::write(
        temp_dir.path().join("gradle.properties"),
        "ktorVersion=2.3.10\nkotlinVersion=2.0.0\n",
    )
    .unwrap();
    let build_gradle = temp_dir.path().join("build.gradle.kts");
    std::fs::write(
        &build_gradle,
        r#"
val ktorVersion: String by project
val kotlinVersion = "2.1.0"

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-stdlib:$kotlinVersion")
    testImplementation("io.ktor:ktor-server-test-host:$ktorVersion")
}
"#,
    )
    .unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);
    assert_eq!(package_data.dependencies.len(), 2);
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/org.jetbrains.kotlin/kotlin-stdlib@2.1.0")
            && dependency.extracted_requirement.as_deref() == Some("2.1.0")
            && dependency.scope.as_deref() == Some("implementation")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/io.ktor/ktor-server-test-host@2.3.10")
            && dependency.extracted_requirement.as_deref() == Some("2.3.10")
            && dependency.scope.as_deref() == Some("testImplementation")
    }));
}

#[test]
fn test_conditional_dependencies_inside_if_blocks_are_extracted() {
    let temp_dir = tempdir().unwrap();
    let build_gradle = temp_dir.path().join("build.gradle");
    std::fs::write(
        &build_gradle,
        r#"
def jscFlavor = 'io.github.react-native-community:jsc-android:2026004.+'

dependencies {
    implementation("com.facebook.react:react-android")

    if (hermesEnabled.toBoolean()) {
        implementation("com.facebook.react:hermes-android")
    } else {
        implementation jscFlavor
    }
}
"#,
    )
    .unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);

    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/com.facebook.react/react-android")
            && dependency.scope.as_deref() == Some("implementation")
    }));
    assert!(package_data.dependencies.iter().any(|dependency| {
        dependency.purl.as_deref() == Some("pkg:maven/com.facebook.react/hermes-android")
            && dependency.scope.as_deref() == Some("implementation")
    }));
}

#[test]
fn test_compile_only_is_not_runtime() {
    let content = r#"
dependencies {
    compileOnly 'org.antlr:antlr:2.7.7'
    compileOnlyApi 'com.example:annotations:1.0.0'
    testCompileOnly 'junit:junit:4.13'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);

    assert_eq!(deps.len(), 3);
    assert_eq!(deps[0].scope, Some("compileOnly".to_string()));
    assert_eq!(deps[0].is_runtime, Some(false));
    assert_eq!(deps[0].is_optional, Some(false));

    assert_eq!(deps[1].scope, Some("compileOnlyApi".to_string()));
    assert_eq!(deps[1].is_runtime, Some(false));
    assert_eq!(deps[1].is_optional, Some(false));

    assert_eq!(deps[2].scope, Some("testCompileOnly".to_string()));
    assert_eq!(deps[2].is_runtime, Some(false));
    assert_eq!(deps[2].is_optional, Some(true));
}

#[test]
fn test_version_catalog_alias_resolution_from_libs_versions_toml() {
    let temp_dir = tempdir().unwrap();
    let gradle_dir = temp_dir.path().join("gradle");
    std::fs::create_dir_all(&gradle_dir).unwrap();

    std::fs::write(
        gradle_dir.join("libs.versions.toml"),
        r#"
[versions]
androidxAppcompat = "1.7.0"

[libraries]
androidx-appcompat = { module = "androidx.appcompat:appcompat", version.ref = "androidxAppcompat" }
guardianproject-panic = { group = "info.guardianproject", name = "panic", version = "1.0.0" }
"#,
    )
    .unwrap();

    let build_gradle = temp_dir.path().join("build.gradle");
    std::fs::write(
        &build_gradle,
        r#"
dependencies {
    implementation libs.androidx.appcompat
    fullImplementation libs.guardianproject.panic
}
"#,
    )
    .unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);

    assert_eq!(package_data.dependencies.len(), 2);
    assert_eq!(
        package_data.dependencies[0].purl,
        Some("pkg:maven/androidx.appcompat/appcompat@1.7.0".to_string())
    );
    assert_eq!(
        package_data.dependencies[0].scope,
        Some("implementation".to_string())
    );
    assert_eq!(
        package_data.dependencies[1].purl,
        Some("pkg:maven/info.guardianproject/panic@1.0.0".to_string())
    );
    assert_eq!(
        package_data.dependencies[1].scope,
        Some("fullImplementation".to_string())
    );
}

#[test]
fn test_extract_gradle_license_metadata_from_pom_block() {
    let content = r#"
plugins {
    id 'java-library'
    id 'maven'
}

dependencies {
    api 'org.apache.commons:commons-text:1.1'
}

configure(install.repositories.mavenInstaller) {
    pom.project {
        licenses {
            license {
                name 'The Apache License, Version 2.0'
                url 'http://www.apache.org/licenses/LICENSE-2.0.txt'
            }
        }
    }
}
"#;

    let temp_dir = tempdir().unwrap();
    let build_gradle = temp_dir.path().join("build.gradle");
    std::fs::write(&build_gradle, content).unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);

    assert_eq!(
            package_data.extracted_license_statement,
            Some(
                "- license:\n    name: The Apache License, Version 2.0\n    url: http://www.apache.org/licenses/LICENSE-2.0.txt\n"
                    .to_string()
            )
        );
    assert_eq!(
        package_data.declared_license_expression_spdx,
        Some("Apache-2.0".to_string())
    );
}

#[test]
fn test_gradle_license_resolves_descriptive_name_via_shared_normalizer() {
    // The classic ASF declared name "The Apache Software License, Version 2.0"
    // was missed by the previous hardcoded patterns (which only matched the
    // literal "apache license, version 2.0"). The shared name/url normalizer
    // resolves it, matching Maven's handling of the same declared name.
    let content = r#"
plugins {
    id 'java-library'
}

configure(install.repositories.mavenInstaller) {
    pom.project {
        licenses {
            license {
                name 'The Apache Software License, Version 2.0'
            }
        }
    }
}
"#;

    let temp_dir = tempdir().unwrap();
    let build_gradle = temp_dir.path().join("build.gradle");
    std::fs::write(&build_gradle, content).unwrap();

    let package_data = GradleParser::extract_first_package(&build_gradle);

    assert_eq!(
        package_data.declared_license_expression.as_deref(),
        Some("apache-2.0")
    );
    assert_eq!(
        package_data.declared_license_expression_spdx.as_deref(),
        Some("Apache-2.0")
    );
}

#[test]
fn test_parse_gradle_version_catalog_helper() {
    let temp_dir = tempdir().unwrap();
    let catalog_path = temp_dir.path().join("libs.versions.toml");
    std::fs::write(
        &catalog_path,
        r#"
[versions]
androidxAppcompat = "1.7.0"

[libraries]
androidx-appcompat = { module = "androidx.appcompat:appcompat", version.ref = "androidxAppcompat" }
"#,
    )
    .unwrap();

    let entries = parse_gradle_version_catalog(&catalog_path).unwrap();
    let entry = entries.get("androidx.appcompat").unwrap();

    assert_eq!(entry.namespace, "androidx.appcompat");
    assert_eq!(entry.name, "appcompat");
    assert_eq!(entry.version.as_deref(), Some("1.7.0"));
}

#[test]
fn test_string_interpolation() {
    let content = r#"
dependencies {
    compile "com.amazonaws:aws-java-sdk-core:${awsVer}"
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].extracted_requirement, Some("${awsVer}".to_string()));
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.amazonaws/aws-java-sdk-core@%24%7BawsVer%7D".to_string())
    );
}

#[test]
fn test_multi_value_string_notation() {
    let content = r#"
dependencies {
    runtimeOnly 'org.springframework:spring-core:2.5',
            'org.springframework:spring-aop:2.5'
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 2);
    assert_eq!(deps[0].scope, Some("".to_string()));
    assert_eq!(deps[1].scope, Some("".to_string()));
}

#[test]
fn test_kotlin_quoted_scope_string_dependency_extracted() {
    let content = r#"
dependencies {
    "js"("jquery:jquery:3.2.1@js")
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].scope, Some("js".to_string()));
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/jquery/jquery@3.2.1%40js".to_string())
    );
}

#[test]
fn test_kotlin_quoted_scope_project_reference_extracted() {
    let content = r#"
subprojects {
    dependencies {
        "testImplementation"(project(":utils:test-utils"))
    }
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].scope, Some("testImplementation".to_string()));
    assert_eq!(deps[0].purl, Some("pkg:maven/utils/test-utils".to_string()));
    assert_eq!(deps[0].is_runtime, Some(false));
    assert_eq!(deps[0].is_optional, Some(true));
    assert_eq!(
        deps[0]
            .extra_data
            .as_ref()
            .and_then(|data| data.get("project_path"))
            .and_then(|value| value.as_str()),
        Some("utils:test-utils")
    );
}

#[test]
fn test_kotlin_quoted_scope_string_dependency_with_closure_extracted() {
    let content = r#"
dependencies {
    "implementation"("com.badlogicgames.gdx:gdx-tools:1.14.0") {
        exclude("com.badlogicgames.gdx", "gdx-backend-lwjgl")
    }
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(deps[0].scope, Some("implementation".to_string()));
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/com.badlogicgames.gdx/gdx-tools@1.14.0".to_string())
    );
}

#[test]
fn test_closure_after_dependency() {
    let content = r#"
dependencies {
    runtimeOnly('org.hibernate:hibernate:3.0.5') {
        transitive = true
    }
}
"#;
    let tokens = lex(content);
    let deps = extract_dependencies(&tokens);
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0].purl,
        Some("pkg:maven/org.hibernate/hibernate@3.0.5".to_string())
    );
    assert_eq!(deps[0].scope, Some("runtimeOnly".to_string()));
}
