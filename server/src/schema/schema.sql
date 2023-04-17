create table clients (
    pubkey     bytea                    not null,
    registered timestamp with time zone not null,
    last_login timestamp with time zone,
    constraint clients_pk
        primary key (pubkey)
);

create table peer_backups (
    id              bigserial,
    source          bytea,
    destination     bytea,
    size_negotiated bigint,
    timestamp       timestamp with time zone,
    constraint peer_backups_pk
        primary key (id),
    constraint source_clients_pubkey_fk
        foreign key (source) references clients,
    constraint destination_clients_pubkey_fk
        foreign key (destination) references clients
);

create table metadata (
    key   varchar not null,
    value varchar,
    constraint metadata_pk
        primary key (key)
);

create table snapshots (
      id            bigserial,
      client_pubkey bytea not null,
      snapshot_hash bytea not null,
      timestamp     timestamp with time zone not null,
      foreign key (client_pubkey) references clients (pubkey)
          match simple on update no action on delete no action,
      constraint snapshots_pk
          primary key (id)
);

create index snapshots_client_pubkey_index on snapshots using btree (client_pubkey);
