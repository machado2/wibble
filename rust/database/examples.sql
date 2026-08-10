CREATE TABLE examples AS
select id, user_input, title, description, content from wibble.content 
where model = 'gpt-4' 
and content is not null
and content like '%<GeneratedImage%';

ALTER TABLE examples ADD COLUMN new_id INT AUTO_INCREMENT PRIMARY KEY;

CREATE INDEX idx_uuid ON examples(id);



select * from examples;

-- drop table examples;