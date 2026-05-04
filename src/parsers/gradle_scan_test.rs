// SPDX-FileCopyrightText: Provenant contributors
// SPDX-License-Identifier: Apache-2.0

#[cfg(test)]
mod tests {
    use std::fs;

    use super::super::scan_test_utils::{assert_dependency_present, scan_and_assemble};
    use crate::models::DatasourceId;

    #[test]
    fn test_gradle_scan_merges_build_and_lockfile_dependency_surfaces() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let gradle_dir = temp_dir.path().join("gradle");
        fs::create_dir_all(&gradle_dir).expect("create gradle dir");
        fs::copy(
            "testdata/gradle-golden/groovy/version-catalog/build.gradle",
            temp_dir.path().join("build.gradle"),
        )
        .expect("copy build.gradle fixture");
        fs::copy(
            "testdata/gradle-golden/groovy/version-catalog/gradle/libs.versions.toml",
            gradle_dir.join("libs.versions.toml"),
        )
        .expect("copy libs.versions.toml fixture");
        fs::copy(
            "testdata/gradle-lock/basic/gradle.lockfile",
            temp_dir.path().join("gradle.lockfile"),
        )
        .expect("copy gradle.lockfile fixture");

        let (files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert_dependency_present(
            &result.dependencies,
            "pkg:maven/androidx.appcompat/appcompat@1.7.0",
            "build.gradle",
        );
        assert_dependency_present(
            &result.dependencies,
            "pkg:maven/org.springframework.boot/spring-boot-starter-web@2.7.0",
            "gradle.lockfile",
        );

        let build_gradle = files
            .iter()
            .find(|file| file.path.ends_with("/build.gradle"))
            .expect("build.gradle should be scanned");
        let gradle_lockfile = files
            .iter()
            .find(|file| file.path.ends_with("/gradle.lockfile"))
            .expect("gradle.lockfile should be scanned");

        assert!(build_gradle.for_packages.is_empty());
        assert!(gradle_lockfile.for_packages.is_empty());
        assert!(
            build_gradle
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::BuildGradle))
        );
        assert!(
            gradle_lockfile
                .package_data
                .iter()
                .any(|pkg_data| pkg_data.datasource_id == Some(DatasourceId::GradleLockfile))
        );
    }

    #[test]
    fn test_gradle_scan_resolves_buildsrc_kotlin_constants() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let build_src_dir = temp_dir
            .path()
            .join("buildSrc/src/main/java/com/example/buildsrc");
        fs::create_dir_all(&build_src_dir).expect("create buildSrc source dir");
        fs::write(
            build_src_dir.join("GradleDeps.kt"),
            r#"
object GradleDeps {
    object Android {
        private const val version = "8.5.2"
        const val gradlePlugin = "com.android.tools.build:gradle:$version"
    }
}
"#,
        )
        .expect("write GradleDeps.kt");
        fs::write(
            build_src_dir.join("Deps.kt"),
            r#"
object Deps {
    object AndroidX {
        const val core = "androidx.core:core:1.15.0"
    }
}
"#,
        )
        .expect("write Deps.kt");
        fs::write(
            temp_dir.path().join("build.gradle"),
            r#"
buildscript {
    dependencies {
        classpath GradleDeps.Android.gradlePlugin
    }
}

dependencies {
    implementation Deps.AndroidX.core
}
"#,
        )
        .expect("write build.gradle");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert_dependency_present(
            &result.dependencies,
            "pkg:maven/com.android.tools.build/gradle@8.5.2",
            "build.gradle",
        );
        assert_dependency_present(
            &result.dependencies,
            "pkg:maven/androidx.core/core@1.15.0",
            "build.gradle",
        );
    }

    #[test]
    fn test_gradle_scan_resolves_buildsrc_constants_from_sibling_build_root() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        let consumer_dir = workspace_dir.join("consumer");
        let helper_root = workspace_dir.join("shared-build");
        let build_src_dir = helper_root.join("buildSrc/src/main/kotlin/com/example/sharedbuild");

        fs::create_dir_all(&consumer_dir).expect("create consumer dir");
        fs::create_dir_all(&build_src_dir).expect("create helper buildSrc dir");
        fs::write(
            helper_root.join("settings.gradle.kts"),
            "rootProject.name = \"shared-build\"\n",
        )
        .expect("write helper settings.gradle.kts");
        fs::write(
            build_src_dir.join("Deps.kt"),
            r#"
object Deps {
    object Libraries {
        const val core = "com.example.libs:core:1.2.3"
    }
}
"#,
        )
        .expect("write sibling Deps.kt");
        fs::write(
            consumer_dir.join("build.gradle"),
            r#"
dependencies {
    implementation Deps.Libraries.core
}
"#,
        )
        .expect("write consumer build.gradle");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert_dependency_present(
            &result.dependencies,
            "pkg:maven/com.example.libs/core@1.2.3",
            "consumer/build.gradle",
        );
    }

    #[test]
    fn test_gradle_scan_does_not_fall_back_to_sibling_buildsrc_when_local_buildsrc_exists() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        let consumer_dir = workspace_dir.join("consumer");
        let local_build_src_dir = consumer_dir.join("buildSrc/src/main/kotlin/com/example/local");
        let helper_root = workspace_dir.join("shared-build");
        let sibling_build_src_dir =
            helper_root.join("buildSrc/src/main/kotlin/com/example/sharedbuild");

        fs::create_dir_all(&local_build_src_dir).expect("create local buildSrc dir");
        fs::create_dir_all(&sibling_build_src_dir).expect("create sibling buildSrc dir");
        fs::write(
            local_build_src_dir.join("Placeholder.kt"),
            r#"
class Placeholder
"#,
        )
        .expect("write local Placeholder.kt");
        fs::write(
            helper_root.join("settings.gradle.kts"),
            "rootProject.name = \"shared-build\"\n",
        )
        .expect("write helper settings.gradle.kts");
        fs::write(
            sibling_build_src_dir.join("Deps.kt"),
            r#"
object Deps {
    object Libraries {
        const val core = "com.example.libs:core:1.2.3"
    }
}
"#,
        )
        .expect("write sibling Deps.kt");
        fs::write(
            consumer_dir.join("build.gradle"),
            r#"
dependencies {
    implementation Deps.Libraries.core
}
"#,
        )
        .expect("write consumer build.gradle");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert!(
            result.dependencies.is_empty(),
            "unexpected dependencies: {:?}",
            result.dependencies
        );
    }

    #[test]
    fn test_gradle_scan_keeps_sibling_buildsrc_conflicts_unresolved() {
        let temp_dir = tempfile::TempDir::new().expect("create temp dir");
        let workspace_dir = temp_dir.path().join("workspace");
        let consumer_dir = workspace_dir.join("consumer");
        let helper_alpha = workspace_dir.join("shared-build-alpha");
        let helper_beta = workspace_dir.join("shared-build-beta");
        let alpha_build_src_dir = helper_alpha.join("buildSrc/src/main/kotlin/com/example/alpha");
        let beta_build_src_dir = helper_beta.join("buildSrc/src/main/kotlin/com/example/beta");

        fs::create_dir_all(&consumer_dir).expect("create consumer dir");
        fs::create_dir_all(&alpha_build_src_dir).expect("create alpha buildSrc dir");
        fs::create_dir_all(&beta_build_src_dir).expect("create beta buildSrc dir");
        fs::write(
            helper_alpha.join("settings.gradle"),
            "rootProject.name = 'shared-build-alpha'\n",
        )
        .expect("write alpha settings.gradle");
        fs::write(
            helper_beta.join("settings.gradle.kts"),
            "rootProject.name = \"shared-build-beta\"\n",
        )
        .expect("write beta settings.gradle.kts");
        fs::write(
            alpha_build_src_dir.join("Deps.kt"),
            r#"
object Deps {
    object Libraries {
        const val core = "com.example.alpha:core:1.0.0"
    }
}
"#,
        )
        .expect("write alpha Deps.kt");
        fs::write(
            beta_build_src_dir.join("Deps.kt"),
            r#"
object Deps {
    object Libraries {
        const val core = "com.example.beta:core:2.0.0"
    }
}
"#,
        )
        .expect("write beta Deps.kt");
        fs::write(
            consumer_dir.join("build.gradle"),
            r#"
dependencies {
    implementation Deps.Libraries.core
}
"#,
        )
        .expect("write consumer build.gradle");

        let (_files, result) = scan_and_assemble(temp_dir.path());

        assert!(result.packages.is_empty());
        assert!(
            result.dependencies.is_empty(),
            "unexpected dependencies: {:?}",
            result.dependencies
        );
    }
}
