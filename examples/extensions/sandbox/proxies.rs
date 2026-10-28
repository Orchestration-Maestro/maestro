//! Owned loopback listeners and their connections, run by the shared background owner.
use super::{
    network::NetworkConfig,
    proxy::{self, Protocol},
};
use maestro_extensions_wasm::{Background, BackgroundTask, ExtensionFuture, wasi};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    rc::Rc,
};
use wasi::sockets::types::{IpAddressFamily, IpSocketAddress, Ipv4SocketAddress, TcpSocket};

/// Live connection handles, reclaimed as each connection finishes.
struct Connections {
    /// The existing event-tail executor.
    background: Background,
    /// Identity for the next admitted connection.
    next: Cell<u64>,
    /// A pending announcement or the task it started.
    tasks: RefCell<HashMap<u64, Option<BackgroundTask>>>,
}
impl Connections {
    /// Announces one peer and retires its handle when forwarding finishes.
    async fn admit(
        self: &Rc<Self>,
        socket: TcpSocket,
        protocol: Protocol,
        rules: Rc<NetworkConfig>,
    ) -> Result<(), String> {
        let id = self.next.get();
        self.next.set(
            id.checked_add(1)
                .ok_or("Proxy connection identities exhausted")?,
        );
        self.tasks.borrow_mut().insert(id, None);
        let owner = Rc::clone(self);
        let work = Box::pin(async move {
            let result = proxy::serve(socket, protocol, &rules).await;
            let retired = owner.tasks.borrow_mut().remove(&id);
            drop(retired);
            result
        });
        let task = self.background.spawn(work).await;
        match task {
            Ok(task) => {
                if let Some(slot) = self.tasks.borrow_mut().get_mut(&id) {
                    *slot = Some(task);
                }
            }
            Err(error) => {
                self.tasks.borrow_mut().remove(&id);
                return Err(error);
            }
        }
        Ok(())
    }
}

/// Two listeners and all their active connections; dropping this value closes every socket.
pub(crate) struct Proxies {
    /// The actual loopback ports selected by the operating system.
    pub(crate) ports: [u16; 2],
    /// Listener tasks, retained independently of connection completion.
    listeners: Vec<BackgroundTask>,
    /// The tasks belonging to these listeners.
    connections: Rc<Connections>,
}
impl Proxies {
    /// Starts both proxy protocols without installing another scheduler or runtime.
    pub(crate) async fn start(
        background: Background,
        rules: NetworkConfig,
    ) -> Result<Self, String> {
        let connections = Rc::new(Connections {
            background,
            next: Cell::new(0),
            tasks: RefCell::default(),
        });
        let mut owner = Self {
            ports: [0, 0],
            listeners: Vec::new(),
            connections,
        };
        let rules = Rc::new(rules);
        for (index, protocol) in [Protocol::Http, Protocol::Socks].into_iter().enumerate() {
            let supplied = match protocol {
                Protocol::Http => rules.http_proxy_port,
                Protocol::Socks => rules.socks_proxy_port,
            };
            if let Some(port) = supplied {
                owner.ports[index] = port;
                continue;
            }

            let (port, work) =
                listener(protocol, Rc::clone(&rules), Rc::clone(&owner.connections))?;
            let task = owner.connections.background.spawn(work).await?;
            owner.ports[index] = port;
            owner.listeners.push(task);
        }
        Ok(owner)
    }
}
impl Drop for Proxies {
    fn drop(&mut self) {
        self.listeners.clear();
        let retired = std::mem::take(&mut *self.connections.tasks.borrow_mut());
        drop(retired);
    }
}

/// Binds an ephemeral loopback port before returning the work that accepts its peers.
fn listener(
    protocol: Protocol,
    rules: Rc<NetworkConfig>,
    connections: Rc<Connections>,
) -> Result<(u16, ExtensionFuture<'static, ()>), String> {
    let socket = TcpSocket::create(IpAddressFamily::Ipv4).map_err(|error| format!("{error:?}"))?;
    socket
        .bind(IpSocketAddress::Ipv4(Ipv4SocketAddress {
            port: 0,
            address: (127, 0, 0, 1),
        }))
        .map_err(|error| format!("{error:?}"))?;
    let mut incoming = socket.listen().map_err(|error| format!("{error:?}"))?;
    let IpSocketAddress::Ipv4(local) = socket
        .get_local_address()
        .map_err(|error| format!("{error:?}"))?
    else {
        return Err("Proxy listener did not bind IPv4 loopback".into());
    };
    let work = Box::pin(async move {
        let _socket = socket;
        while let Some(peer) = incoming.next().await {
            connections.admit(peer, protocol, Rc::clone(&rules)).await?;
        }
        Ok(())
    });
    Ok((local.port, work))
}
