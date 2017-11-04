error_chain! {
    errors {
        NoHomeDirectory {
            description("Could not locate home directory.")
        }
        NoCrateEntryPoint {
            description("No crate entry found.")
        }
        NoCrateDirectoryProvided {
            description("No crate source directory was provided.")
        }
        NoSearchQuery {
            description("No search query was provided.")
        }
        NoSuchDirectory(directory: String) {
            description("no such directory")
            display("Couldn't find directory: {}", directory)
        }

        /// The dependency could not be found.
        CrateParseError(krate: String, err: String) {
            description("crate could not be parsed")
            display("Failed to parse crate {}: {}", krate, err)
        }
        NameEncodingError(name: String) {
            description("Failed to encode name: {}")
        }
    }
}
