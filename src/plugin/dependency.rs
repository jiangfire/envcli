//! Plugin dependency management
//!
//! Responsible for parsing and managing dependencies between plugins

use crate::plugin::types::PluginMetadata;
use std::collections::{HashMap, VecDeque};

/// Dependency resolution error
#[derive(Debug, thiserror::Error)]
pub enum DependencyError {
    #[error("Circular dependency detected: {0}")]
    CircularDependency(String),

    #[error("Missing dependency: {0}")]
    MissingDependency(String),

    #[error("Dependency resolution failed: {0}")]
    ResolutionFailed(String),
}

/// Dependency resolver
pub struct DependencyResolver;

impl DependencyResolver {
    /// Resolve dependencies and return correct loading order (topological sort)
    pub fn resolve(
        plugins: &HashMap<String, PluginMetadata>,
    ) -> Result<Vec<String>, DependencyError> {
        // 1. Build dependency graph and in-degree
        let mut graph: HashMap<String, Vec<String>> = HashMap::new();
        let mut in_degree: HashMap<String, usize> = HashMap::new();

        // Initialize in-degree for all plugins
        for (id, metadata) in plugins {
            in_degree.entry(id.clone()).or_insert(0);

            // Check if dependencies exist
            for dep in &metadata.dependencies {
                if !plugins.contains_key(dep) {
                    return Err(DependencyError::MissingDependency(format!(
                        "{} depends on {} but {} does not exist",
                        id, dep, dep
                    )));
                }

                graph.entry(dep.clone()).or_default().push(id.clone());
                *in_degree.entry(id.clone()).or_insert(0) += 1;
            }
        }

        // 2. Topological sort (Kahn's algorithm)
        let mut queue: VecDeque<String> = VecDeque::new();
        let mut result: Vec<String> = Vec::new();

        // Find all nodes with in-degree 0
        for (id, degree) in &in_degree {
            if *degree == 0 {
                queue.push_back(id.clone());
            }
        }

        // Process queue
        while let Some(current) = queue.pop_front() {
            result.push(current.clone());

            // Find all plugins that depend on current plugin
            if let Some(dependents) = graph.get(&current) {
                for dependent in dependents {
                    let degree = in_degree.get_mut(dependent).unwrap();
                    *degree -= 1;

                    if *degree == 0 {
                        queue.push_back(dependent.clone());
                    }
                }
            }
        }

        // 3. Check for circular dependencies
        if result.len() != plugins.len() {
            let missing: Vec<String> = plugins
                .keys()
                .filter(|id| !result.contains(id))
                .cloned()
                .collect();

            return Err(DependencyError::CircularDependency(format!(
                "Circular dependency detected, involving plugins: {}",
                missing.join(", ")
            )));
        }

        Ok(result)
    }

    /// Check if there are circular dependencies
    pub fn has_cycle(plugins: &HashMap<String, PluginMetadata>) -> bool {
        Self::resolve(plugins).is_err()
    }

    /// Get all dependencies of a plugin (including indirect dependencies)
    pub fn get_all_dependencies(
        plugins: &HashMap<String, PluginMetadata>,
        plugin_id: &str,
    ) -> Vec<String> {
        let mut all_deps = Vec::new();
        let mut visited = std::collections::HashSet::new();
        let mut stack = vec![plugin_id.to_string()];

        while let Some(current) = stack.pop() {
            if visited.contains(&current) {
                continue;
            }
            visited.insert(current.clone());

            if let Some(metadata) = plugins.get(&current) {
                for dep in &metadata.dependencies {
                    if !all_deps.contains(dep) {
                        all_deps.push(dep.clone());
                        stack.push(dep.clone());
                    }
                }
            }
        }

        all_deps
    }

    /// Validate dependency relationships
    pub fn validate_dependencies(
        plugins: &HashMap<String, PluginMetadata>,
    ) -> Result<(), DependencyError> {
        // Check if all dependencies exist
        for (id, metadata) in plugins {
            for dep in &metadata.dependencies {
                if !plugins.contains_key(dep) {
                    return Err(DependencyError::MissingDependency(format!(
                        "{} depends on non-existent plugin: {}",
                        id, dep
                    )));
                }
            }
        }

        // Check for circular dependencies
        if Self::has_cycle(plugins) {
            return Err(DependencyError::CircularDependency(
                "Circular dependency found in dependency graph".to_string(),
            ));
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::plugin::types::{Platform, PluginType};

    #[test]
    fn test_simple_dependency() {
        let mut plugins = HashMap::new();

        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["a".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let order = DependencyResolver::resolve(&plugins).unwrap();
        assert_eq!(order, vec!["a", "b"]);
    }

    #[test]
    fn test_circular_dependency() {
        let mut plugins = HashMap::new();

        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["b".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["a".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let result = DependencyResolver::resolve(&plugins);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DependencyError::CircularDependency(_)
        ));
    }

    #[test]
    fn test_missing_dependency() {
        let mut plugins = HashMap::new();

        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["x".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let result = DependencyResolver::resolve(&plugins);
        assert!(result.is_err());
        assert!(matches!(
            result.unwrap_err(),
            DependencyError::MissingDependency(_)
        ));
    }

    #[test]
    fn test_get_all_dependencies() {
        let mut plugins = HashMap::new();

        // a -> b -> c
        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["b".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["c".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "c".to_string(),
            PluginMetadata {
                id: "c".to_string(),
                name: "C".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let deps = DependencyResolver::get_all_dependencies(&plugins, "a");
        assert_eq!(deps.len(), 2);
        assert!(deps.contains(&"b".to_string()));
        assert!(deps.contains(&"c".to_string()));
    }

    #[test]
    fn test_validate_dependencies() {
        let mut plugins = HashMap::new();

        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["a".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let result = DependencyResolver::validate_dependencies(&plugins);
        assert!(result.is_ok());
    }

    #[test]
    fn test_has_cycle() {
        let mut plugins = HashMap::new();

        // No cycle
        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        assert!(!DependencyResolver::has_cycle(&plugins));

        // Add cycle
        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["a".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins
            .get_mut("a")
            .unwrap()
            .dependencies
            .push("b".to_string());

        assert!(DependencyResolver::has_cycle(&plugins));
    }

    #[test]
    fn test_complex_dependency_graph() {
        let mut plugins = HashMap::new();

        // Create complex graph:
        // a -> b -> d
        // a -> c -> d
        // e (independent)

        plugins.insert(
            "a".to_string(),
            PluginMetadata {
                id: "a".to_string(),
                name: "A".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["b".to_string(), "c".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "b".to_string(),
            PluginMetadata {
                id: "b".to_string(),
                name: "B".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["d".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "c".to_string(),
            PluginMetadata {
                id: "c".to_string(),
                name: "C".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec!["d".to_string()],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "d".to_string(),
            PluginMetadata {
                id: "d".to_string(),
                name: "D".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        plugins.insert(
            "e".to_string(),
            PluginMetadata {
                id: "e".to_string(),
                name: "E".to_string(),
                version: "1.0.0".to_string(),
                description: None,
                author: None,
                plugin_type: PluginType::DynamicLibrary,
                hooks: vec![],
                extensions: vec![],
                config_schema: None,
                enabled: true,
                dependencies: vec![],
                platforms: vec![Platform::current()],
                envcli_version: None,
                signature: None,
            },
        );

        let order = DependencyResolver::resolve(&plugins).unwrap();

        // d should come before b and c
        let d_pos = order.iter().position(|x| x == "d").unwrap();
        let b_pos = order.iter().position(|x| x == "b").unwrap();
        let c_pos = order.iter().position(|x| x == "c").unwrap();

        assert!(d_pos < b_pos);
        assert!(d_pos < c_pos);

        // b and c should come before a
        let a_pos = order.iter().position(|x| x == "a").unwrap();
        assert!(b_pos < a_pos);
        assert!(c_pos < a_pos);

        // e can be anywhere
        assert!(order.contains(&"e".to_string()));
    }
}
