-- ═══════════════════════════════════════════════════════════════════════════
--  Terrapolis v0.4.1 — акаунти, друзі, Rich Presence і спільні збірки (Supabase)
--
--  Виконай увесь файл у Supabase → SQL Editor (можна запускати повторно: скрипт
--  ідемпотентний; таблиці зі старою структурою переносяться в схему
--  tp_legacy_<дата_час>, а не видаляються). Лаунчер ходить у базу лише з publishable (anon) ключем, тому:
--    • RLS увімкнено на всіх таблицях і БЕЗ політик, а права anon/authenticated
--      на таблиці відкликано — напряму (REST /rest/v1/<table>) нічого не прочитати
--      й не змінити, зокрема token_hash;
--    • уся робота йде через RPC-функції SECURITY DEFINER, кожна з яких перевіряє
--      пару (user_id, account_token): у базі зберігається лише SHA-256 токена,
--      і жодна функція не повертає token_hash;
--    • запис у Storage дозволений лише за короткоживучим «квитком»
--      (upload_tickets), який видає publish_pack / unpublish_pack.
--  Архіви збірок лежать у публічному бакеті `shared-packs` за шляхом
--  `<user_id>/<object_key>.zip`, де object_key — секретний випадковий UUID
--  (не дорівнює публічному id збірки й змінюється з кожною публікацією).
--  Шлях видає лише get_pack_download після перевірки дружби й доступу, а лаунчер
--  друга додатково звіряє SHA-256 архіву із записом власника.
-- ═══════════════════════════════════════════════════════════════════════════

create extension if not exists pgcrypto with schema extensions;

-- ───────────────────────────── підготовка бази ─────────────────────────────
-- Якщо в public уже лежать таблиці з такими самими назвами, але іншою структурою
-- (наприклад, з попередньої схеми: friendships.requester_id замість requester),
-- `create table if not exists` їх пропустив би, і скрипт упав би далі. Тому старі
-- таблиці не видаляються, а переносяться в окрему схему tp_legacy_<дата_час>
-- (не відкрита для REST API): дані лишаються, а public звільняється для v0.4.1.
do $$
declare
  v_expected constant jsonb := '{
    "users":          {"id":"uuid","username":"text","token_hash":"text","created_at":"timestamptz","last_seen":"timestamptz"},
    "presence":       {"user_id":"uuid","status":"text","character":"text","pack_name":"text","pack_type":"text",
                       "instance_id":"text","project_code":"text","project_name":"text","shared_pack_id":"uuid",
                       "started_at":"timestamptz","updated_at":"timestamptz"},
    "friendships":    {"id":"uuid","requester":"uuid","addressee":"uuid","status":"text","created_at":"timestamptz"},
    "shared_packs":   {"id":"uuid","owner":"uuid","local_id":"text","object_key":"uuid","name":"text","visibility":"text",
                       "minecraft_version":"text","loader":"text","loader_version":"text","size_bytes":"int8",
                       "sha256":"text","ready":"bool","updated_at":"timestamptz"},
    "pack_requests":  {"id":"uuid","pack_id":"uuid","requester":"uuid","status":"text","created_at":"timestamptz"},
    "upload_tickets": {"path":"text","owner":"uuid","expires_at":"timestamptz"}
  }';
  v_table  text;
  v_cols   jsonb;
  v_col    text;
  v_type   text;
  v_legacy boolean := false;
  v_schema text := 'tp_legacy_' || to_char(clock_timestamp(), 'YYYYMMDD_HH24MISS');
begin
  for v_table, v_cols in select key, value from jsonb_each(v_expected) loop
    continue when to_regclass('public.' || v_table) is null;
    for v_col, v_type in select key, value #>> '{}' from jsonb_each(v_cols) loop
      if not exists (
        select 1 from information_schema.columns c
        where c.table_schema = 'public' and c.table_name = v_table
          and c.column_name = v_col and c.udt_name = v_type
      ) then
        raise notice 'Terrapolis: public.% has no column "%" of type %', v_table, v_col, v_type;
        v_legacy := true;
      end if;
    end loop;
  end loop;

  -- Таблиці пов'язані зовнішніми ключами, тож переносимо весь набір разом.
  if v_legacy then
    execute format('create schema %I', v_schema);
    execute format('revoke all on schema %I from public, anon, authenticated', v_schema);
    foreach v_table in array array['upload_tickets', 'pack_requests', 'shared_packs',
                                   'friendships', 'presence', 'users'] loop
      if to_regclass('public.' || v_table) is not null then
        execute format('alter table public.%I set schema %I', v_table, v_schema);
        raise notice 'Terrapolis: old public.% moved to %.%', v_table, v_schema, v_table;
      end if;
    end loop;
  end if;
end;
$$;

-- Старі версії RPC з тими самими назвами, але іншими параметрами чи типом результату
-- ламають `create or replace function` (і створюють неоднозначні перевантаження
-- для PostgREST). Видаляємо всі варіанти — нижче вони створюються наново разом із
-- правами й політиками Storage, що на них посилаються.
do $$
declare
  r record;
begin
  for r in
    select p.proname, pg_get_function_identity_arguments(p.oid) as args
    from pg_proc p join pg_namespace n on n.oid = p.pronamespace
    where n.nspname = 'public'
      and p.proname in ('tp_auth', 'tp_are_friends', 'tp_clean', 'can_upload_pack',
                        'register_account', 'sign_out', 'update_presence', 'social_snapshot',
                        'search_users', 'send_friend_request', 'respond_friend_request',
                        'remove_friend', 'publish_pack', 'finish_pack_upload', 'unpublish_pack',
                        'request_pack_access', 'respond_pack_request', 'get_pack_download')
  loop
    execute format('drop function public.%I(%s) cascade', r.proname, r.args);
  end loop;
end;
$$;

-- Сторонні політики на бакет shared-packs (наприклад, «дозволити всім завантажувати»)
-- обійшли б квитки: політики в Postgres об'єднуються через OR. Лишаємо лише свої.
do $$
declare
  r record;
begin
  for r in
    select policyname from pg_policies
    where schemaname = 'storage' and tablename = 'objects'
      and policyname not like 'tp\_shared\_packs\_%'
      and (coalesce(qual, '') || ' ' || coalesce(with_check, '')) like '%shared-packs%'
  loop
    execute format('drop policy %I on storage.objects', r.policyname);
    raise notice 'Terrapolis: removed foreign storage policy "%" on shared-packs', r.policyname;
  end loop;
end;
$$;

-- ───────────────────────────── таблиці ─────────────────────────────

create table if not exists public.users (
  id          uuid primary key default gen_random_uuid(),
  username    text not null check (username ~ '^[A-Za-z0-9_]{3,20}$'),
  token_hash  text not null check (token_hash ~ '^[0-9a-f]{64}$'),
  created_at  timestamptz not null default now(),
  last_seen   timestamptz not null default now()
);
-- Унікальність без урахування регістру: «User» і «user» — одне ім'я.
create unique index if not exists users_username_lower on public.users (lower(username));

create table if not exists public.presence (
  user_id         uuid primary key references public.users (id) on delete cascade,
  status          text not null default 'offline' check (status in ('online', 'playing', 'offline')),
  character       text not null default '',
  pack_name       text not null default '',
  pack_type       text not null default ''
                  check (pack_type in ('', 'project', 'custom_private', 'custom_public', 'custom_request')),
  instance_id     text not null default '',
  project_code    text not null default '',
  project_name    text not null default '',
  shared_pack_id  uuid,
  started_at      timestamptz,
  updated_at      timestamptz not null default now()
);

create table if not exists public.friendships (
  id          uuid primary key default gen_random_uuid(),
  requester   uuid not null references public.users (id) on delete cascade,
  addressee   uuid not null references public.users (id) on delete cascade,
  status      text not null default 'pending' check (status in ('pending', 'accepted')),
  created_at  timestamptz not null default now(),
  check (requester <> addressee)
);
create unique index if not exists friendships_pair
  on public.friendships (least(requester, addressee), greatest(requester, addressee));
create index if not exists friendships_addressee on public.friendships (addressee);

create table if not exists public.shared_packs (
  id                 uuid primary key default gen_random_uuid(),
  owner              uuid not null references public.users (id) on delete cascade,
  local_id           text not null check (local_id ~ '^[A-Za-z0-9_.-]{1,64}$'),
  object_key         uuid not null default gen_random_uuid(),
  name               text not null,
  visibility         text not null check (visibility in ('public', 'on_request')),
  minecraft_version  text not null default '',
  loader             text not null default '',
  loader_version     text not null default '',
  size_bytes         bigint not null default 0 check (size_bytes between 0 and 47185920),
  sha256             text not null default '',
  ready              boolean not null default false,
  updated_at         timestamptz not null default now(),
  unique (owner, local_id)
);

create table if not exists public.pack_requests (
  id          uuid primary key default gen_random_uuid(),
  pack_id     uuid not null references public.shared_packs (id) on delete cascade,
  requester   uuid not null references public.users (id) on delete cascade,
  status      text not null default 'pending' check (status in ('pending', 'approved', 'denied')),
  created_at  timestamptz not null default now(),
  unique (pack_id, requester)
);

create table if not exists public.upload_tickets (
  path        text primary key,
  owner       uuid not null references public.users (id) on delete cascade,
  expires_at  timestamptz not null
);

alter table public.users          enable row level security;
alter table public.presence       enable row level security;
alter table public.friendships    enable row level security;
alter table public.shared_packs   enable row level security;
alter table public.pack_requests  enable row level security;
alter table public.upload_tickets enable row level security;

revoke all on table public.users, public.presence, public.friendships,
                    public.shared_packs, public.pack_requests, public.upload_tickets
  from anon, authenticated;

-- ───────────────────────────── допоміжні функції ─────────────────────────────

create or replace function public.tp_auth(p_user uuid, p_token text)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
begin
  if p_user is null or p_token is null or p_token !~ '^[0-9a-f]{64}$' or not exists (
    select 1 from public.users
    where id = p_user
      and token_hash = encode(extensions.digest(p_token, 'sha256'), 'hex')
  ) then
    raise exception 'auth_failed' using errcode = '28000';
  end if;
  update public.users set last_seen = now() where id = p_user;
end;
$$;

create or replace function public.tp_are_friends(a uuid, b uuid)
returns boolean
language sql
stable
security definer
set search_path = public
as $$
  select exists (
    select 1 from public.friendships
    where status = 'accepted'
      and least(requester, addressee) = least(a, b)
      and greatest(requester, addressee) = greatest(a, b)
  );
$$;

-- Текст від клієнта без керівних символів (переноси, нульові байти, bidi-підміни).
create or replace function public.tp_clean(p_text text)
returns text
language sql
immutable
set search_path = public
as $$
  select regexp_replace(coalesce(p_text, ''), '[[:cntrl:]‎‏‪-‮⁦-⁩]', '', 'g');
$$;

-- Перевірка для політик Storage: на цей шлях виданий чинний квиток.
create or replace function public.can_upload_pack(p_path text)
returns boolean
language sql
stable
security definer
set search_path = public
as $$
  select exists (
    select 1 from public.upload_tickets
    where path = p_path and expires_at > now()
  );
$$;

revoke execute on function public.tp_auth(uuid, text) from public, anon, authenticated;
revoke execute on function public.tp_are_friends(uuid, uuid) from public, anon, authenticated;
revoke execute on function public.tp_clean(text) from public, anon, authenticated;
grant execute on function public.can_upload_pack(text) to anon, authenticated;

-- ───────────────────────────── акаунт ─────────────────────────────

-- Реєстрація @username. Якщо ім'я (без урахування регістру) вже належить цьому ж токену —
-- повертає наявний акаунт (повторний вхід на тому ж комп'ютері). Інакше — username_taken.
create or replace function public.register_account(p_username text, p_token_hash text)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_id uuid;
  v_hash text;
  v_name text;
begin
  if p_username is null or p_username !~ '^[A-Za-z0-9_]{3,20}$' then
    raise exception 'invalid_username';
  end if;
  if p_token_hash is null or p_token_hash !~ '^[0-9a-f]{64}$' then
    raise exception 'invalid_token';
  end if;

  select id, token_hash, username into v_id, v_hash, v_name
  from public.users where lower(username) = lower(p_username);

  if v_id is not null then
    if v_hash = p_token_hash then
      update public.users set last_seen = now() where id = v_id;
      insert into public.presence (user_id, status) values (v_id, 'online')
        on conflict (user_id) do update set status = 'online', updated_at = now();
      return json_build_object('id', v_id, 'username', v_name, 'created', false);
    end if;
    raise exception 'username_taken';
  end if;

  begin
    insert into public.users (username, token_hash) values (p_username, p_token_hash)
      returning id into v_id;
  exception when unique_violation then
    raise exception 'username_taken';
  end;
  insert into public.presence (user_id, status) values (v_id, 'online');
  return json_build_object('id', v_id, 'username', p_username, 'created', true);
end;
$$;

create or replace function public.sign_out(p_user uuid, p_token text)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
begin
  perform public.tp_auth(p_user, p_token);
  update public.presence
     set status = 'offline', pack_name = '', pack_type = '', instance_id = '',
         project_code = '', project_name = '', shared_pack_id = null,
         started_at = null, updated_at = now()
   where user_id = p_user;
end;
$$;

-- ───────────────────────────── Rich Presence ─────────────────────────────

create or replace function public.update_presence(
  p_user uuid,
  p_token text,
  p_status text,
  p_character text default '',
  p_pack_name text default '',
  p_pack_type text default '',
  p_instance_id text default '',
  p_project_code text default '',
  p_project_name text default '',
  p_shared_pack_id uuid default null,
  p_started_at timestamptz default null
)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_type text := coalesce(p_pack_type, '');
  v_character text := left(regexp_replace(coalesce(p_character, ''), '[^A-Za-z0-9_]', '', 'g'), 16);
  v_started timestamptz;
begin
  perform public.tp_auth(p_user, p_token);
  if p_status not in ('online', 'playing', 'offline') then
    raise exception 'invalid_status';
  end if;

  if p_status <> 'playing' then
    insert into public.presence (user_id, status, character, updated_at)
    values (p_user, p_status, v_character, now())
    on conflict (user_id) do update
      set status = excluded.status, character = excluded.character,
          pack_name = '', pack_type = '', instance_id = '', project_code = '',
          project_name = '', shared_pack_id = null, started_at = null, updated_at = now();
    return json_build_object('server_now', now());
  end if;

  if v_type not in ('project', 'custom_private', 'custom_public', 'custom_request') then
    raise exception 'invalid_pack_type';
  end if;
  -- Код проєкту потрапляє в connect_project друзів — лише безпечні символи.
  if v_type = 'project' and coalesce(p_project_code, '') !~ '^[A-Za-z0-9_./-]{1,120}$' then
    raise exception 'invalid_project_code';
  end if;
  if coalesce(p_instance_id, '') !~ '^[A-Za-z0-9_.-]{0,64}$' then
    raise exception 'invalid_instance';
  end if;
  -- Час старту — з годинника сервера (без розсинхрону годинників друзів), але сесія,
  -- що вже триває (перезапуск лаунчера під час гри), зберігає свій справжній старт.
  v_started := case
    when p_started_at is null or p_started_at > now() - interval '2 minutes' then now()
    else greatest(p_started_at, now() - interval '24 hours')
  end;

  insert into public.presence as pr (
    user_id, status, character, pack_name, pack_type, instance_id,
    project_code, project_name, shared_pack_id, started_at, updated_at
  ) values (
    p_user, 'playing', v_character,
    case when v_type = 'custom_private' then '' else left(public.tp_clean(p_pack_name), 80) end,
    v_type, coalesce(p_instance_id, ''),
    case when v_type = 'project' then p_project_code else '' end,
    case when v_type = 'project' then left(public.tp_clean(p_project_name), 80) else '' end,
    -- Чужий id збірки сюди підставити можна, але social_snapshot показує лише збірки власника.
    case when v_type in ('custom_public', 'custom_request') then p_shared_pack_id else null end,
    v_started, now()
  )
  on conflict (user_id) do update
    set status = 'playing',
        character = excluded.character,
        pack_name = excluded.pack_name,
        pack_type = excluded.pack_type,
        instance_id = excluded.instance_id,
        project_code = excluded.project_code,
        project_name = excluded.project_name,
        shared_pack_id = excluded.shared_pack_id,
        started_at = case
          when pr.status = 'playing' and pr.instance_id = excluded.instance_id and pr.started_at is not null
            then pr.started_at
          else excluded.started_at
        end,
        updated_at = now();
  return json_build_object('server_now', now());
end;
$$;

-- ───────────────────────────── друзі ─────────────────────────────

-- Усе для вкладки «Друзі» одним запитом: друзі зі статусом (присутність старша за 150 с
-- вважається офлайном — лаунчер міг закритися аварійно), заявки та запити на доступ.
create or replace function public.social_snapshot(p_user uuid, p_token text)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_result json;
begin
  perform public.tp_auth(p_user, p_token);
  select json_build_object(
    'server_now', now(),
    'friends', coalesce((
      select json_agg(f order by f.rank, lower(f.username))
      from (
        select
          u.id as user_id,
          u.username,
          s.status,
          case s.status when 'playing' then 0 when 'online' then 1 else 2 end as rank,
          case when s.status = 'playing' then p.character else '' end as character,
          case when s.status = 'playing' then p.pack_name else '' end as pack_name,
          case when s.status = 'playing' then p.pack_type else '' end as pack_type,
          case when s.status = 'playing' then p.instance_id else '' end as instance_id,
          case when s.status = 'playing' then p.project_code else '' end as project_code,
          case when s.status = 'playing' then p.project_name else '' end as project_name,
          case when s.status = 'playing' and sp.id is not null then sp.id end as shared_pack_id,
          case when s.status = 'playing' then p.started_at end as started_at,
          case
            when s.status <> 'playing' or sp.id is null then 'none'
            when sp.visibility = 'public' then 'public'
            else coalesce(rq.status, 'none')
          end as access,
          u.last_seen,
          coalesce((
            select json_agg(json_build_object(
              'id', x.id, 'name', x.name, 'visibility', x.visibility,
              'minecraft_version', x.minecraft_version, 'loader', x.loader,
              'size_bytes', x.size_bytes,
              'access', case when x.visibility = 'public' then 'public'
                             else coalesce((select r.status from public.pack_requests r
                                            where r.pack_id = x.id and r.requester = p_user), 'none') end
            ) order by x.updated_at desc)
            from public.shared_packs x
            where x.owner = u.id and x.ready
          ), '[]'::json) as packs
        from public.friendships fr
        join public.users u
          on u.id = case when fr.requester = p_user then fr.addressee else fr.requester end
        left join public.presence p on p.user_id = u.id
        cross join lateral (
          select case
            when p.user_id is null or p.updated_at < now() - interval '150 seconds' then 'offline'
            else p.status
          end as status
        ) s
        left join public.shared_packs sp
          on sp.id = p.shared_pack_id and sp.owner = u.id and sp.ready
        left join public.pack_requests rq
          on rq.pack_id = sp.id and rq.requester = p_user
        where fr.status = 'accepted' and p_user in (fr.requester, fr.addressee)
      ) f
    ), '[]'::json),
    'incoming', coalesce((
      select json_agg(json_build_object('id', fr.id, 'user_id', u.id, 'username', u.username,
                                        'created_at', fr.created_at) order by fr.created_at desc)
      from public.friendships fr join public.users u on u.id = fr.requester
      where fr.addressee = p_user and fr.status = 'pending'
    ), '[]'::json),
    'outgoing', coalesce((
      select json_agg(json_build_object('id', fr.id, 'user_id', u.id, 'username', u.username,
                                        'created_at', fr.created_at) order by fr.created_at desc)
      from public.friendships fr join public.users u on u.id = fr.addressee
      where fr.requester = p_user and fr.status = 'pending'
    ), '[]'::json),
    'pack_requests', coalesce((
      select json_agg(json_build_object('id', r.id, 'pack_id', sp.id, 'pack_name', sp.name,
                                        'user_id', u.id, 'username', u.username,
                                        'created_at', r.created_at) order by r.created_at desc)
      from public.pack_requests r
      join public.shared_packs sp on sp.id = r.pack_id
      join public.users u on u.id = r.requester
      where sp.owner = p_user and r.status = 'pending'
    ), '[]'::json)
  ) into v_result;
  return v_result;
end;
$$;

-- Пошук за префіксом @username. Запит очищається до [A-Za-z0-9_], а порівняння — без LIKE
-- (у LIKE «_» і «%» — шаблони: «a_» знаходило б «ab»). PostgREST-фільтри тут не беруть участі.
create or replace function public.search_users(p_user uuid, p_token text, p_query text)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_query text := left(lower(regexp_replace(coalesce(p_query, ''), '[^A-Za-z0-9_]', '', 'g')), 20);
begin
  perform public.tp_auth(p_user, p_token);
  if length(v_query) < 2 then
    return '[]'::json;
  end if;
  return coalesce((
    select json_agg(json_build_object('user_id', r.user_id, 'username', r.username, 'relation', r.relation)
                    order by r.exact desc, length(r.username), lower(r.username))
    from (
      select u.id as user_id, u.username,
             lower(u.username) = v_query as exact,
             case
               when fr.id is null then 'none'
               when fr.status = 'accepted' then 'friend'
               when fr.requester = p_user then 'outgoing'
               else 'incoming'
             end as relation
      from public.users u
      left join public.friendships fr
        on least(fr.requester, fr.addressee) = least(u.id, p_user)
       and greatest(fr.requester, fr.addressee) = greatest(u.id, p_user)
      where u.id <> p_user and left(lower(u.username), length(v_query)) = v_query
      order by lower(u.username) = v_query desc, length(u.username)
      limit 8
    ) r
  ), '[]'::json);
end;
$$;

create or replace function public.send_friend_request(p_user uuid, p_token text, p_username text)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_target uuid;
  v_row public.friendships%rowtype;
begin
  perform public.tp_auth(p_user, p_token);
  select id into v_target from public.users
   where lower(username) = lower(ltrim(coalesce(p_username, ''), '@'));
  if v_target is null then
    raise exception 'user_not_found';
  end if;
  if v_target = p_user then
    raise exception 'cannot_add_self';
  end if;

  select * into v_row from public.friendships
   where least(requester, addressee) = least(p_user, v_target)
     and greatest(requester, addressee) = greatest(p_user, v_target);

  if found then
    if v_row.status = 'accepted' then
      return json_build_object('status', 'already_friends');
    elsif v_row.requester = p_user then
      return json_build_object('status', 'already_sent');
    else
      -- Зустрічна заявка: друг уже запросив нас — просто приймаємо.
      update public.friendships set status = 'accepted' where id = v_row.id;
      return json_build_object('status', 'accepted');
    end if;
  end if;

  if (select count(*) from public.friendships where p_user in (requester, addressee)) >= 300 then
    raise exception 'too_many';
  end if;
  begin
    insert into public.friendships (requester, addressee) values (p_user, v_target);
  exception when unique_violation then
    -- Подвійний клік / зустрічна заявка в ту саму мить.
    return json_build_object('status', 'already_sent');
  end;
  return json_build_object('status', 'sent');
end;
$$;

-- Прийняти / відхилити можна лише заявку, адресовану саме тобі.
create or replace function public.respond_friend_request(p_user uuid, p_token text, p_request uuid, p_accept boolean)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
begin
  perform public.tp_auth(p_user, p_token);
  if p_accept then
    update public.friendships set status = 'accepted'
     where id = p_request and addressee = p_user and status = 'pending';
  else
    delete from public.friendships
     where id = p_request and addressee = p_user and status = 'pending';
  end if;
  if not found then
    raise exception 'request_not_found';
  end if;
end;
$$;

-- Видалити друга або скасувати власну заявку. Доступи до збірок між вами теж скасовуються.
create or replace function public.remove_friend(p_user uuid, p_token text, p_friend uuid)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
begin
  perform public.tp_auth(p_user, p_token);
  delete from public.friendships
   where least(requester, addressee) = least(p_user, p_friend)
     and greatest(requester, addressee) = greatest(p_user, p_friend);
  delete from public.pack_requests r
   using public.shared_packs sp
   where sp.id = r.pack_id
     and ((sp.owner = p_user and r.requester = p_friend)
       or (sp.owner = p_friend and r.requester = p_user));
end;
$$;

-- ───────────────────────────── спільні збірки ─────────────────────────────

-- Крок 1 публікації: запис у shared_packs (ready = false) + квиток на завантаження архіву.
-- Повертає шлях для Storage і (якщо був) старий шлях, який треба прибрати.
create or replace function public.publish_pack(
  p_user uuid,
  p_token text,
  p_local_id text,
  p_name text,
  p_visibility text,
  p_minecraft_version text,
  p_loader text,
  p_loader_version text,
  p_size bigint,
  p_sha256 text
)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_id uuid;
  v_old_key uuid;
  v_key uuid := gen_random_uuid();
  v_path text;
  v_old_path text;
begin
  perform public.tp_auth(p_user, p_token);
  if p_visibility not in ('public', 'on_request') then
    raise exception 'invalid_visibility';
  end if;
  if p_local_id is null or p_local_id !~ '^[A-Za-z0-9_.-]{1,64}$' then
    raise exception 'invalid_pack';
  end if;
  if p_size is null or p_size < 0 or p_size > 47185920 then
    raise exception 'pack_too_large';
  end if;
  if p_sha256 is null or p_sha256 !~ '^[0-9a-f]{64}$' then
    raise exception 'invalid_hash';
  end if;
  if (select count(*) from public.shared_packs where owner = p_user and local_id <> p_local_id) >= 30 then
    raise exception 'too_many_packs';
  end if;

  select object_key into v_old_key from public.shared_packs
   where owner = p_user and local_id = p_local_id;

  insert into public.shared_packs as sp (
    owner, local_id, object_key, name, visibility, minecraft_version, loader,
    loader_version, size_bytes, sha256, ready, updated_at
  ) values (
    p_user, p_local_id, v_key, left(coalesce(nullif(trim(public.tp_clean(p_name)), ''), 'Modpack'), 40), p_visibility,
    left(public.tp_clean(p_minecraft_version), 40), left(public.tp_clean(p_loader), 20),
    left(public.tp_clean(p_loader_version), 60), p_size, p_sha256, false, now()
  )
  on conflict (owner, local_id) do update
    set object_key = excluded.object_key, name = excluded.name, visibility = excluded.visibility,
        minecraft_version = excluded.minecraft_version, loader = excluded.loader,
        loader_version = excluded.loader_version, size_bytes = excluded.size_bytes,
        sha256 = excluded.sha256, ready = false, updated_at = now()
  returning id into v_id;

  delete from public.upload_tickets where expires_at < now();
  v_path := p_user::text || '/' || v_key::text || '.zip';
  insert into public.upload_tickets (path, owner, expires_at)
  values (v_path, p_user, now() + interval '15 minutes')
  on conflict (path) do update set expires_at = excluded.expires_at;

  if v_old_key is not null then
    -- Старий шлях могли бачити друзі: квиток лише на 2 хвилини (щоб лаунчер встиг видалити архів).
    v_old_path := p_user::text || '/' || v_old_key::text || '.zip';
    insert into public.upload_tickets (path, owner, expires_at)
    values (v_old_path, p_user, now() + interval '2 minutes')
    on conflict (path) do update set expires_at = excluded.expires_at;
  end if;

  return json_build_object('id', v_id, 'path', v_path, 'old_path', v_old_path);
end;
$$;

-- Крок 2 публікації: архів завантажено — збірка стає видимою друзям, квиток гаситься.
create or replace function public.finish_pack_upload(p_user uuid, p_token text, p_pack uuid)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_key uuid;
begin
  perform public.tp_auth(p_user, p_token);
  update public.shared_packs set ready = true, updated_at = now()
   where id = p_pack and owner = p_user
   returning object_key into v_key;
  if v_key is null then
    raise exception 'pack_not_found';
  end if;
  delete from public.upload_tickets where path = p_user::text || '/' || v_key::text || '.zip';
end;
$$;

-- Збірка стала приватною (або видалена): прибираємо запис (разом із запитами доступу)
-- і видаємо 2-хвилинний квиток на видалення архіву зі Storage.
create or replace function public.unpublish_pack(p_user uuid, p_token text, p_local_id text)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_key uuid;
  v_path text;
begin
  perform public.tp_auth(p_user, p_token);
  delete from public.shared_packs where owner = p_user and local_id = p_local_id
    returning object_key into v_key;
  if v_key is null then
    return json_build_object('path', null);
  end if;
  v_path := p_user::text || '/' || v_key::text || '.zip';
  insert into public.upload_tickets (path, owner, expires_at)
  values (v_path, p_user, now() + interval '2 minutes')
  on conflict (path) do update set expires_at = excluded.expires_at;
  return json_build_object('path', v_path);
end;
$$;

create or replace function public.request_pack_access(p_user uuid, p_token text, p_pack uuid)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_pack public.shared_packs%rowtype;
  v_status text;
begin
  perform public.tp_auth(p_user, p_token);
  select * into v_pack from public.shared_packs where id = p_pack and ready;
  if not found then
    raise exception 'pack_not_found';
  end if;
  if not public.tp_are_friends(p_user, v_pack.owner) then
    raise exception 'not_friends';
  end if;
  if v_pack.visibility = 'public' then
    return json_build_object('status', 'public');
  end if;
  -- Після відмови повторно попросити можна не раніше ніж за 10 хвилин (без спаму власнику).
  if exists (select 1 from public.pack_requests
              where pack_id = p_pack and requester = p_user and status = 'denied'
                and created_at > now() - interval '10 minutes') then
    raise exception 'request_cooldown';
  end if;
  insert into public.pack_requests as r (pack_id, requester)
  values (p_pack, p_user)
  on conflict (pack_id, requester) do update
    set status = case when r.status = 'approved' then 'approved' else 'pending' end,
        created_at = case when r.status in ('approved', 'pending') then r.created_at else now() end
  returning status into v_status;
  return json_build_object('status', v_status);
end;
$$;

-- Схвалити / відхилити можна лише запит до ВЛАСНОЇ збірки.
create or replace function public.respond_pack_request(p_user uuid, p_token text, p_request uuid, p_approve boolean)
returns void
language plpgsql
security definer
set search_path = public, extensions
as $$
begin
  perform public.tp_auth(p_user, p_token);
  update public.pack_requests r
     set status = case when p_approve then 'approved' else 'denied' end,
         created_at = now()
    from public.shared_packs sp
   where r.id = p_request and sp.id = r.pack_id and sp.owner = p_user;
  if not found then
    raise exception 'request_not_found';
  end if;
end;
$$;

-- Шлях до архіву — лише власнику, або другу для публічної збірки / зі схваленим доступом.
create or replace function public.get_pack_download(p_user uuid, p_token text, p_pack uuid)
returns json
language plpgsql
security definer
set search_path = public, extensions
as $$
declare
  v_pack public.shared_packs%rowtype;
  v_owner text;
begin
  perform public.tp_auth(p_user, p_token);
  select * into v_pack from public.shared_packs where id = p_pack and ready;
  if not found then
    raise exception 'pack_not_found';
  end if;
  if v_pack.owner <> p_user then
    if not public.tp_are_friends(p_user, v_pack.owner) then
      raise exception 'not_friends';
    end if;
    if v_pack.visibility = 'on_request' and not exists (
      select 1 from public.pack_requests
       where pack_id = p_pack and requester = p_user and status = 'approved'
    ) then
      raise exception 'access_denied';
    end if;
  end if;
  select username into v_owner from public.users where id = v_pack.owner;
  return json_build_object(
    'id', v_pack.id,
    'name', v_pack.name,
    'owner_username', v_owner,
    'path', v_pack.owner::text || '/' || v_pack.object_key::text || '.zip',
    'size_bytes', v_pack.size_bytes,
    'sha256', v_pack.sha256,
    'minecraft_version', v_pack.minecraft_version,
    'loader', v_pack.loader,
    'loader_version', v_pack.loader_version
  );
end;
$$;

-- Postgres за замовчуванням дає EXECUTE усім (PUBLIC): спершу забираємо, потім видаємо
-- лише потрібні RPC ролям anon / authenticated.
revoke execute on function
  public.register_account(text, text),
  public.sign_out(uuid, text),
  public.update_presence(uuid, text, text, text, text, text, text, text, text, uuid, timestamptz),
  public.social_snapshot(uuid, text),
  public.search_users(uuid, text, text),
  public.send_friend_request(uuid, text, text),
  public.respond_friend_request(uuid, text, uuid, boolean),
  public.remove_friend(uuid, text, uuid),
  public.publish_pack(uuid, text, text, text, text, text, text, text, bigint, text),
  public.finish_pack_upload(uuid, text, uuid),
  public.unpublish_pack(uuid, text, text),
  public.request_pack_access(uuid, text, uuid),
  public.respond_pack_request(uuid, text, uuid, boolean),
  public.get_pack_download(uuid, text, uuid)
from public;

grant execute on function
  public.register_account(text, text),
  public.sign_out(uuid, text),
  public.update_presence(uuid, text, text, text, text, text, text, text, text, uuid, timestamptz),
  public.social_snapshot(uuid, text),
  public.search_users(uuid, text, text),
  public.send_friend_request(uuid, text, text),
  public.respond_friend_request(uuid, text, uuid, boolean),
  public.remove_friend(uuid, text, uuid),
  public.publish_pack(uuid, text, text, text, text, text, text, text, bigint, text),
  public.finish_pack_upload(uuid, text, uuid),
  public.unpublish_pack(uuid, text, text),
  public.request_pack_access(uuid, text, uuid),
  public.respond_pack_request(uuid, text, uuid, boolean),
  public.get_pack_download(uuid, text, uuid)
to anon, authenticated;

-- ───────────────────────────── Storage: бакет shared-packs ─────────────────────────────

insert into storage.buckets (id, name, public, file_size_limit, allowed_mime_types)
values ('shared-packs', 'shared-packs', true, 47185920, array['application/zip'])
on conflict (id) do update
  set public = true, file_size_limit = 47185920, allowed_mime_types = array['application/zip'];

-- Публічне читання (`/object/public/...`) працює без політик (потрібен секретний шлях).
-- Політики нижче керують записом, перезаписом (x-upsert), видаленням і переліком через API:
-- усе — лише для шляху з чинним квитком, тож перелічити чи перезаписати чужі архіви не можна.
drop policy if exists "tp_shared_packs_insert" on storage.objects;
drop policy if exists "tp_shared_packs_update" on storage.objects;
drop policy if exists "tp_shared_packs_delete" on storage.objects;
drop policy if exists "tp_shared_packs_select" on storage.objects;

create policy "tp_shared_packs_insert" on storage.objects
  for insert to anon, authenticated
  with check (bucket_id = 'shared-packs' and public.can_upload_pack(name));

create policy "tp_shared_packs_update" on storage.objects
  for update to anon, authenticated
  using (bucket_id = 'shared-packs' and public.can_upload_pack(name))
  with check (bucket_id = 'shared-packs' and public.can_upload_pack(name));

create policy "tp_shared_packs_delete" on storage.objects
  for delete to anon, authenticated
  using (bucket_id = 'shared-packs' and public.can_upload_pack(name));

create policy "tp_shared_packs_select" on storage.objects
  for select to anon, authenticated
  using (bucket_id = 'shared-packs' and public.can_upload_pack(name));
