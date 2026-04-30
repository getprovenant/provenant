val ktorVersion: String by project

dependencies {
    "implementation"("io.ktor:ktor-client-core:$ktorVersion")

    "implementation"("com.badlogicgames.gdx:gdx-tools:1.14.0") {
        exclude("com.badlogicgames.gdx", "gdx-backend-lwjgl")
    }

    "testImplementation"(project(":utils:test-utils"))
}
