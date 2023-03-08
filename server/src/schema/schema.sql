create table clients
(
    pubkey     bytea                    not null,
    registered timestamp with time zone not null,
    last_login timestamp with time zone,
    constraint clients_pk
        primary key (pubkey)
);

create table peer_backups
(
    column_name     bigserial,
    source          bytea,
    destination     bytea,
    size_negotiated bigint,
    constraint peer_backups_pk
        primary key (column_name),
    constraint source_clients_pubkey_fk
        foreign key (source) references clients,
    constraint destination_clients_pubkey_fk
        foreign key (destination) references clients
);

create table metadata
(
    key   varchar not null,
    value varchar,
    constraint metadata_pk
        primary key (key)
);
