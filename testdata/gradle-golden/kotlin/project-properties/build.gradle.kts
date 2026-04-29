// inspired by repositories that keep versions in gradle.properties and bind them with `by project`
val kotlinVersion: String by project
val ktorVersion: String by project

dependencies {
    implementation("org.jetbrains.kotlin:kotlin-stdlib:$kotlinVersion")
    testImplementation("io.ktor:ktor-server-test-host:$ktorVersion")
}
