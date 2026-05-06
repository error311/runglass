use std::collections::HashMap;
use std::process::Command;

use anyhow::{anyhow, Context, Result};

use crate::{
    DockerContainerChange, DockerImageChange, DockerNetworkChange, DockerPortChange,
    DockerSnapshot, DockerSummary, DockerVolumeChange,
};
fn docker_lines(subcommand: &str, args: &[&str]) -> Result<Vec<String>> {
    let output = Command::new("docker")
        .arg(subcommand)
        .args(args)
        .output()
        .with_context(|| format!("failed to run docker {}", subcommand))?;
    if !output.status.success() {
        return Err(anyhow!(
            "docker {} failed: {}",
            subcommand,
            String::from_utf8_lossy(&output.stderr).trim()
        ));
    }
    Ok(String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter(|line| !line.trim().is_empty())
        .map(ToString::to_string)
        .collect())
}

fn parse_docker_container(line: &str) -> Option<DockerContainerChange> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let name = value
        .get("Names")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return None;
    }
    let image = value
        .get("Image")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let state = value
        .get("State")
        .or_else(|| value.get("Status"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let ports = value
        .get("Ports")
        .and_then(|value| value.as_str())
        .map(split_csv_field)
        .unwrap_or_default();

    Some(DockerContainerChange {
        name,
        image,
        state,
        ports,
        mounts: Vec::new(),
    })
}

fn inspect_docker_container(name: &str) -> Option<DockerContainerChange> {
    let output = Command::new("docker")
        .arg("inspect")
        .arg(name)
        .output()
        .ok()?;
    if !output.status.success() {
        return None;
    }

    let value: serde_json::Value = serde_json::from_slice(&output.stdout).ok()?;
    let object = value.as_array()?.first()?;
    let image = object
        .get("Config")
        .and_then(|value| value.get("Image"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let state = object
        .get("State")
        .and_then(|value| value.get("Status"))
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    let mounts = object
        .get("Mounts")
        .and_then(|value| value.as_array())
        .map(|mounts| {
            mounts
                .iter()
                .filter_map(|mount| {
                    let source = mount
                        .get("Name")
                        .or_else(|| mount.get("Source"))
                        .and_then(|value| value.as_str())?;
                    let destination = mount.get("Destination").and_then(|value| value.as_str())?;
                    Some(format!("{source}:{destination}"))
                })
                .collect()
        })
        .unwrap_or_default();

    let ports = object
        .get("NetworkSettings")
        .and_then(|value| value.get("Ports"))
        .and_then(|value| value.as_object())
        .map(|ports| {
            let mut mappings = Vec::new();
            for (container_port, binding) in ports {
                let Some(bindings) = binding.as_array() else {
                    continue;
                };
                for binding in bindings {
                    let host_ip = binding
                        .get("HostIp")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    let host_port = binding
                        .get("HostPort")
                        .and_then(|value| value.as_str())
                        .unwrap_or("");
                    if !host_port.is_empty() {
                        let host = if host_ip.is_empty() {
                            host_port.to_string()
                        } else {
                            format!("{host_ip}:{host_port}")
                        };
                        mappings.push(format!("{host}->{container_port}"));
                    }
                }
            }
            mappings
        })
        .unwrap_or_default();

    Some(DockerContainerChange {
        name: name.to_string(),
        image,
        state,
        ports,
        mounts,
    })
}

fn parse_docker_image(line: &str) -> Option<DockerImageChange> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let repository = value
        .get("Repository")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    let tag = value
        .get("Tag")
        .and_then(|value| value.as_str())
        .unwrap_or_default();
    if repository.is_empty() || tag.is_empty() {
        return None;
    }
    Some(DockerImageChange {
        tag: format!("{repository}:{tag}"),
        digest: value
            .get("Digest")
            .and_then(|value| value.as_str())
            .map(ToString::to_string),
    })
}

fn parse_docker_volume(line: &str) -> Option<DockerVolumeChange> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let name = value
        .get("Name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(DockerVolumeChange {
        name,
        mountpoint: None,
    })
}

fn parse_docker_network(line: &str) -> Option<DockerNetworkChange> {
    let value: serde_json::Value = serde_json::from_str(line).ok()?;
    let name = value
        .get("Name")
        .and_then(|value| value.as_str())
        .unwrap_or_default()
        .to_string();
    if name.is_empty() {
        return None;
    }
    Some(DockerNetworkChange {
        name,
        driver: value
            .get("Driver")
            .and_then(|value| value.as_str())
            .unwrap_or_default()
            .to_string(),
    })
}

fn split_csv_field(value: &str) -> Vec<String> {
    value
        .split(',')
        .map(str::trim)
        .filter(|segment| !segment.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn extract_published_ports(container: &DockerContainerChange) -> Vec<DockerPortChange> {
    container
        .ports
        .iter()
        .filter_map(|port| parse_published_port(port))
        .collect()
}

fn parse_published_port(value: &str) -> Option<DockerPortChange> {
    let mapping = value.replace(' ', "");
    let (host, container) = mapping.split_once("->")?;
    let (host_ip, host_port) = if let Some((ip, port)) = host.rsplit_once(':') {
        (ip.to_string(), port.parse().ok()?)
    } else {
        ("".to_string(), host.parse().ok()?)
    };
    let (container_port, protocol) = container.split_once('/')?;
    Some(DockerPortChange {
        host_ip,
        host_port,
        container_port: container_port.parse().ok()?,
        protocol: protocol.to_string(),
    })
}

#[cfg(target_os = "linux")]
pub fn capture_docker_snapshot() -> Result<DockerSnapshot> {
    let mut containers = HashMap::new();
    for line in docker_lines("ps", &["-a", "--format", "{{json .}}"])? {
        if let Some(container) = parse_docker_container(&line) {
            let detailed = inspect_docker_container(&container.name).unwrap_or(container);
            containers.insert(detailed.name.clone(), detailed);
        }
    }

    let mut images = HashMap::new();
    for line in docker_lines("images", &["--digests", "--format", "{{json .}}"])? {
        if let Some(image) = parse_docker_image(&line) {
            images.insert(image.tag.clone(), image);
        }
    }

    let mut volumes = HashMap::new();
    for line in docker_lines("volume", &["ls", "--format", "{{json .}}"])? {
        if let Some(volume) = parse_docker_volume(&line) {
            volumes.insert(volume.name.clone(), volume);
        }
    }

    let mut networks = HashMap::new();
    for line in docker_lines("network", &["ls", "--format", "{{json .}}"])? {
        if let Some(network) = parse_docker_network(&line) {
            networks.insert(network.name.clone(), network);
        }
    }

    Ok(DockerSnapshot {
        containers,
        images,
        volumes,
        networks,
    })
}

pub fn diff_docker_snapshots(before: &DockerSnapshot, after: &DockerSnapshot) -> DockerSummary {
    let mut containers_created = Vec::new();
    let mut containers_removed = Vec::new();
    let mut containers_changed = Vec::new();
    let mut images_pulled = Vec::new();
    let mut volumes_created = Vec::new();
    let mut networks_created = Vec::new();
    let mut ports_published = Vec::new();

    for (name, container) in &after.containers {
        match before.containers.get(name) {
            None => {
                containers_created.push(container.clone());
                ports_published.extend(extract_published_ports(container));
            }
            Some(previous) if previous != container => {
                containers_changed.push(container.clone());
                ports_published.extend(extract_published_ports(container));
            }
            _ => {}
        }
    }

    for (name, container) in &before.containers {
        if !after.containers.contains_key(name) {
            containers_removed.push(container.clone());
        }
    }

    for (tag, image) in &after.images {
        if !before.images.contains_key(tag) {
            images_pulled.push(image.clone());
        }
    }

    for (name, volume) in &after.volumes {
        if !before.volumes.contains_key(name) {
            volumes_created.push(volume.clone());
        }
    }

    for (name, network) in &after.networks {
        if !before.networks.contains_key(name) {
            networks_created.push(network.clone());
        }
    }

    DockerSummary {
        containers_created,
        containers_removed,
        containers_changed,
        images_pulled,
        volumes_created,
        networks_created,
        ports_published,
    }
}
