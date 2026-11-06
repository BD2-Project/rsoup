# Versionado del driver rsoup

Convención propia del driver, **independiente** de los números de versión del gestor SoupDB y del frontend SoupChef.

## Publicación

- **crates.io**: paquete **`soupdb-driver`** (el nombre `rsoup` ya estaba ocupado). El nombre de **librería** se mantiene como `rsoup`, por lo que los consumidores usan `use rsoup::...`.
- La publicación la ejecuta `.github/workflows/release.yml` al empujar un tag `vX.Y.Z`; requiere el secret de repositorio `CARGO_REGISTRY_TOKEN`.

## Esquema

- SemVer simplificado `vX.Y.Z` mediante **tags anotados** de Git.
- `X.Y`: bump de **minor** para cada entrega con cambios de contrato (protocolo, API pública o tipos).
- `Z`: bump de **patch** para fixes publicados entre entregas.
- `Cargo.toml` debe reflejar la versión del último tag.
- El tag se crea sobre el commit de merge del PR de publicación a `main`.

## Flujo de trabajo

1. Rama `feat|fix|docs/<tema>` desde `main`.
2. PR a `main` con CI en verde (`fmt`, `clippy`, `test`, `e2e`).
3. Publicación: merge del PR y tag anotado inmediato.

## Mapa de tags

| Tag | Fecha | Cambio clave |
|---|---|---|
| `v0.1.0` | — | Protocolo v1: 6 tipos nativos, transacciones, POINT, TUI/E2E |
| `v0.2.0` | 2026-11-06 | Protocolo v1.1 (`type_name`/`encoding`), VECTOR + tipos extensibles (fallback opaco), prepared statements, transporte TLS opt-in |

## Matriz de compatibilidad rsoup ↔ gestor

| rsoup | Gestor (protocolo) | Notas |
|---|---|---|
| `0.1.0` | v1 (`type_code` numérico) | 6 tipos nativos (`INT, FLOAT, VARCHAR, TEXT, BOOL, POINT`) |
| `0.2.0` | v1.1 (`type_name` + `encoding`, prepared statements, VECTOR, fallback opaco) | Objetivo de la reestructuración de plugins del gestor |

> La especificación canónica del wire protocol vive en `docs/protocolo.md` del repo del gestor. Si el codec Rust y el Python difieren, manda la especificación del gestor.
