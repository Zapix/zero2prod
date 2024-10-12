-- Add migration script here
insert into users(user_id, username, password_hash) values (
    'd36ecbf9-bfb7-4732-a677-9c6a6924570f',
    'admin',
    '$argon2id$v=19$m=15000,t=2,p=1$5KlGZPNhOAZzVyTFzIp4Ng$xx+qZJKzTH5u3UaNja8Gmej9cTKIPivff0FNIvX0shE'
);