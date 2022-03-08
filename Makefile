all: release

lint:
	cargo clippy
	cargo check --tests
	cargo check --benches

test:
	cargo test --release -- --test-threads=1

bench:
	cargo bench

fmt:
	cargo +nightly fmt

fmtall:
	bash tools/fmt.sh

clean:
	cargo clean
	git stash
	git clean -fdx

update:
	cargo update

doc:
	cargo doc --open

define collect
	-@ rm -rf $(1)
	mkdir $(1)
	cp \
		./target/$(2)/$(1)/ovr \
		./target/$(2)/$(1)/ovrd \
		$(shell go env GOPATH)/bin/tendermint \
		$(1)/
	cp -f $(1)/* ~/.cargo/bin/
	cd $(1)/ && ./ovrd pack
	cp -f /tmp/ovrd $(1)/
	cp -f /tmp/ovrd ~/.cargo/bin/
endef

build: submod
	cargo build --bins
	$(call collect,debug)

release: build_release

release_rocksdb: build_release_rocksdb

build_release: submod
	cargo build --release --bins
	$(call collect,release)

build_release_rocksdb: submod
	cargo build --release --bins --no-default-features --features="vsdb_rocksdb"
	$(call collect,release)

build_release_musl: submod
	cargo build --release --bins --target=x86_64-unknown-linux-musl
	$(call collect,release,x86_64-unknown-linux-musl)

submod:
	git submodule update --init --recursive

prodenv:
	bash tools/create_prod_env.sh

run_prodenv:
	bash tools/create_prod_env.sh prodenv
	ovr dev -s -n prodenv

start_prodenv:
	ovr dev -s -n prodenv

stop_prodenv:
	ovr dev -S -n prodenv

destroy_prodenv:
	ovr dev -d -n prodenv

show_prodenv:
	ovr dev -i -n prodenv


# CI 
ci_build_binary_rust_base:
	docker build -t binary-rust-base -f container/Dockerfile-binary-rust-base .

ci_build_dev_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-dev .
	
ci_build_release_binary_image:
	sed -i "s/^ENV VERGEN_SHA_EXTERN .*/ENV VERGEN_SHA_EXTERN ${VERGEN_SHA_EXTERN}/g" container/Dockerfile-binary-image-release
	docker build -t ovrd-binary-image:$(IMAGE_TAG) -f container/Dockerfile-binary-image-release .

ci_build_image:
	@ if [ -d "./binary" ]; then \
		rm -rf ./binary || true; \
	fi
	@ docker run --rm -d --name ovrd-binary ovrd-binary-image:$(IMAGE_TAG)
	@ docker cp ovrd-binary:/binary ./binary
	@ docker rm -f ovrd-binary
	@ docker build -t $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) -f container/Dockerfile-goleveldb .
ifeq ($(ENV),release)
	docker tag $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG) $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

ci_push_image:
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker push $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

clean_image:
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:$(IMAGE_TAG)
ifeq ($(ENV),release)
	docker rmi $(PUBLIC_ECR_URL)/$(ENV)/ovrd:latest
endif

clean_binary_image:
	docker rmi ovrd-binary-image:$(IMAGE_TAG)
